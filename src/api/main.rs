//! REST API server for the weighted random assignment engine.
//!
//! Exposes HTTP endpoints for generating weighted random assignments.
//! External applications (e.g., PHP, Python) can call this service via REST.

use std::net::SocketAddr;

use axum::{
    extract::Json,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use weighted_random_assignment::core::models::{
    AssignmentConfig, AssignmentResult, HistoricalPairing,
};
use weighted_random_assignment::core::penalty::{
    ExponentialPenalty, LinearPenalty, PenaltyStrategy, ThresholdPenalty,
};
use weighted_random_assignment::engine::AssignmentEngine;
use weighted_random_assignment::infra::config::ServerConfig;
use weighted_random_assignment::infra::logging;

/// Maximum number of participants allowed per request.
const MAX_PARTICIPANTS: usize = 10_000;

/// Maximum number of history records allowed per request.
const MAX_HISTORY_RECORDS: usize = 100_000;

/// Maximum length of a single participant name in bytes.
const MAX_PARTICIPANT_NAME_LEN: usize = 256;

/// Maximum request body size in bytes (2 MB).
const MAX_BODY_SIZE: usize = 2 * 1024 * 1024;

/// Request body for the assignment generation endpoint.
#[derive(Debug, Deserialize)]
struct GenerateRequest {
    /// List of participant identifiers.
    participants: Vec<String>,
    /// Historical pairing records.
    #[serde(default)]
    history: Vec<HistoricalPairing>,
    /// The penalty factor (used with linear strategy by default).
    #[serde(default = "default_penalty_factor")]
    penalty_factor: f64,
    /// The penalty strategy to use: "linear", "exponential", or "threshold".
    #[serde(default = "default_strategy_name")]
    strategy: String,
    /// Decay rate for exponential strategy (default: 0.5).
    #[serde(default = "default_decay_rate")]
    decay_rate: f64,
    /// Threshold count for threshold strategy (default: 2).
    #[serde(default = "default_threshold")]
    threshold: u32,
    /// Reduced weight factor for threshold strategy (default: 0.1).
    #[serde(default = "default_reduced_weight_factor")]
    reduced_weight_factor: f64,
    /// Optional seed for deterministic results.
    seed: Option<u64>,
}

fn default_penalty_factor() -> f64 {
    1.0
}

fn default_strategy_name() -> String {
    "linear".to_string()
}

fn default_decay_rate() -> f64 {
    0.5
}

fn default_threshold() -> u32 {
    2
}

fn default_reduced_weight_factor() -> f64 {
    0.1
}

/// Error response body.
#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

/// Health check response.
#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    version: String,
}

#[tokio::main]
async fn main() {
    logging::init();

    let config = ServerConfig::from_env();
    let addr: SocketAddr = config.bind_address().parse().expect("invalid bind address");

    tracing::info!(%addr, "starting API server");

    let app = create_router();

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");

    tracing::info!(%addr, "API server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
}

/// Waits for a SIGTERM or Ctrl+C signal for graceful shutdown.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => { tracing::info!("received Ctrl+C, shutting down"); }
        () = terminate => { tracing::info!("received SIGTERM, shutting down"); }
    }
}

/// Builds a CORS layer based on the `CORS_ALLOW_ORIGIN` environment variable.
///
/// - If set to a specific origin (e.g., `"https://example.com"`), restricts to that origin.
/// - If set to `"*"`, allows all origins (permissive).
/// - If not set, defaults to restrictive CORS (no cross-origin requests allowed).
fn build_cors_layer() -> CorsLayer {
    match std::env::var("CORS_ALLOW_ORIGIN") {
        Ok(ref origin) if origin == "*" => {
            tracing::warn!("CORS_ALLOW_ORIGIN is set to '*'; allowing all origins");
            CorsLayer::permissive()
        }
        Ok(origin) => match origin.parse() {
            Ok(header_value) => CorsLayer::new().allow_origin(AllowOrigin::exact(header_value)),
            Err(e) => {
                tracing::error!(
                    origin = %origin,
                    error = %e,
                    "invalid CORS_ALLOW_ORIGIN value, falling back to restrictive CORS"
                );
                CorsLayer::new()
            }
        },
        Err(_) => CorsLayer::new(),
    }
}

/// Creates the axum router with all routes and middleware.
pub fn create_router() -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/assignments/generate", post(generate_handler))
        .layer(RequestBodyLimitLayer::new(MAX_BODY_SIZE))
        .layer(TraceLayer::new_for_http())
        .layer(build_cors_layer())
}

/// Health check endpoint.
async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Returns a BAD_REQUEST error response.
fn bad_request(msg: String) -> (StatusCode, Json<ErrorResponse>) {
    (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: msg }))
}

/// Assignment generation endpoint.
async fn generate_handler(
    Json(request): Json<GenerateRequest>,
) -> Result<Json<AssignmentResult>, impl IntoResponse> {
    // Validate request size limits
    if request.participants.len() > MAX_PARTICIPANTS {
        return Err(bad_request(format!(
            "too many participants: {}. Maximum is {}",
            request.participants.len(),
            MAX_PARTICIPANTS
        )));
    }
    if request.history.len() > MAX_HISTORY_RECORDS {
        return Err(bad_request(format!(
            "too many history records: {}. Maximum is {}",
            request.history.len(),
            MAX_HISTORY_RECORDS
        )));
    }

    // Validate participant name lengths
    for (i, name) in request.participants.iter().enumerate() {
        if name.len() > MAX_PARTICIPANT_NAME_LEN {
            return Err(bad_request(format!(
                "participant at index {} name is too long ({} bytes). Maximum is {} bytes",
                i,
                name.len(),
                MAX_PARTICIPANT_NAME_LEN
            )));
        }
    }

    // Validate strategy parameters
    let strategy: Box<dyn PenaltyStrategy> = match request.strategy.as_str() {
        "linear" => {
            if request.penalty_factor < 0.0 || !request.penalty_factor.is_finite() {
                return Err(bad_request(
                    "penalty_factor must be non-negative and finite".to_string(),
                ));
            }
            Box::new(LinearPenalty::new(request.penalty_factor))
        }
        "exponential" => {
            if !(0.0..=1.0).contains(&request.decay_rate) {
                return Err(bad_request(
                    "decay_rate must be between 0.0 and 1.0".to_string(),
                ));
            }
            Box::new(ExponentialPenalty::new(request.decay_rate))
        }
        "threshold" => {
            if !(0.0..=1.0).contains(&request.reduced_weight_factor) {
                return Err(bad_request(
                    "reduced_weight_factor must be between 0.0 and 1.0".to_string(),
                ));
            }
            Box::new(ThresholdPenalty::new(
                request.threshold,
                request.reduced_weight_factor,
            ))
        }
        other => {
            return Err(bad_request(format!(
                "unknown strategy: {}. Use 'linear', 'exponential', or 'threshold'",
                other
            )));
        }
    };

    let config = AssignmentConfig::new(
        request.participants,
        request.history,
        strategy,
        request.seed,
    );

    let engine = AssignmentEngine::new();

    match engine.generate(config) {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn app() -> Router {
        create_router()
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = app().await;
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_generate_endpoint() {
        let app = app().await;
        let body = serde_json::json!({
            "participants": ["A", "B", "C", "D"],
            "history": [
                {"giver": "A", "receiver": "B", "count": 2}
            ],
            "penalty_factor": 1.0,
            "seed": 42
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/assignments/generate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_generate_invalid_single_participant() {
        let app = app().await;
        let body = serde_json::json!({
            "participants": ["A"],
            "penalty_factor": 1.0
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/assignments/generate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn test_generate_unknown_strategy() {
        let app = app().await;
        let body = serde_json::json!({
            "participants": ["A", "B", "C"],
            "strategy": "unknown"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/assignments/generate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_generate_exponential_strategy() {
        let app = app().await;
        let body = serde_json::json!({
            "participants": ["A", "B", "C", "D"],
            "strategy": "exponential",
            "decay_rate": 0.5,
            "seed": 42
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/assignments/generate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_generate_threshold_strategy() {
        let app = app().await;
        let body = serde_json::json!({
            "participants": ["A", "B", "C", "D"],
            "strategy": "threshold",
            "threshold": 2,
            "reduced_weight_factor": 0.1,
            "seed": 42
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/assignments/generate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_generate_negative_penalty_factor() {
        let app = app().await;
        let body = serde_json::json!({
            "participants": ["A", "B", "C"],
            "penalty_factor": -1.0
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/assignments/generate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_generate_invalid_decay_rate() {
        let app = app().await;
        let body = serde_json::json!({
            "participants": ["A", "B", "C"],
            "strategy": "exponential",
            "decay_rate": 1.5
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/assignments/generate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_generate_invalid_reduced_weight_factor() {
        let app = app().await;
        let body = serde_json::json!({
            "participants": ["A", "B", "C"],
            "strategy": "threshold",
            "reduced_weight_factor": -0.5
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/assignments/generate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
