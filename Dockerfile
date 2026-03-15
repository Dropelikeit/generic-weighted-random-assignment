# Stage 1: Build
FROM rust:1.83-slim AS builder

WORKDIR /app

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./

# Create dummy source files to build dependencies
RUN mkdir -p src/core src/engine src/api src/cli src/infra && \
    echo "pub mod core; pub mod engine; pub mod infra;" > src/lib.rs && \
    echo "pub mod algorithm; pub mod models; pub mod penalty;" > src/core/mod.rs && \
    echo "" > src/core/algorithm.rs && \
    echo "" > src/core/models.rs && \
    echo "" > src/core/penalty.rs && \
    echo "fn main() {}" > src/api/main.rs && \
    echo "fn main() {}" > src/cli/main.rs && \
    echo "" > src/engine/mod.rs && \
    echo "pub mod config; pub mod logging;" > src/infra/mod.rs && \
    echo "" > src/infra/config.rs && \
    echo "" > src/infra/logging.rs

# Build dependencies only. The dummy source files are intentionally incomplete
# stubs that will fail to compile -- the sole purpose of this step is to
# download and compile third-party crate dependencies so they are cached in
# this Docker layer. The || true ensures the build continues despite the
# expected compilation errors from stub files.
RUN cargo build --release 2>&1 || true

# Copy actual source code
COPY src/ src/

# Touch source files to ensure they're rebuilt
RUN touch src/lib.rs src/api/main.rs src/cli/main.rs

# Build the actual binaries
RUN cargo build --release --bin wra-api --bin wra

# Stage 2: Runtime
FROM debian:bookworm-slim AS runtime

# ca-certificates: HTTPS support; curl: required by docker-compose healthcheck
# (see docker-compose.yml: curl -f http://localhost:8080/health).
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --shell /bin/bash appuser

COPY --from=builder /app/target/release/wra-api /usr/local/bin/
COPY --from=builder /app/target/release/wra /usr/local/bin/

USER appuser

EXPOSE 8080

ENV RUST_LOG=info

CMD ["wra-api"]
