//! Application configuration.

/// Configuration for the API server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// The host to bind to.
    pub host: String,
    /// The port to listen on.
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
        }
    }
}

impl ServerConfig {
    /// Creates a `ServerConfig` from environment variables.
    ///
    /// Reads `HOST` and `PORT` from the environment, falling back to defaults
    /// (`127.0.0.1:8080`) if not set.
    pub fn from_env() -> Self {
        let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = match std::env::var("PORT") {
            Ok(ref val) => match val.parse::<u16>() {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(
                        value = %val,
                        error = %e,
                        "invalid PORT environment variable, falling back to default 8080"
                    );
                    8080
                }
            },
            Err(_) => 8080,
        };

        Self { host, port }
    }

    /// Returns the socket address string for binding.
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
