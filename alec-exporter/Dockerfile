# ALEC Exporter Dockerfile
# Multi-stage build for minimal image size

# Build stage
FROM rust:1.76-bookworm AS builder

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY alec-codec/Cargo.toml alec-codec/Cargo.toml
COPY alec-gateway/Cargo.toml alec-gateway/Cargo.toml
COPY alec-complexity/Cargo.toml alec-complexity/Cargo.toml
COPY alec-testdata/Cargo.toml alec-testdata/Cargo.toml
COPY alec-exporter/Cargo.toml alec-exporter/Cargo.toml

# Create dummy source files for dependency caching
RUN mkdir -p alec-codec/src && echo "fn main() {}" > alec-codec/src/lib.rs && \
    mkdir -p alec-gateway/src && echo "fn main() {}" > alec-gateway/src/lib.rs && \
    mkdir -p alec-complexity/src && echo "fn main() {}" > alec-complexity/src/lib.rs && \
    mkdir -p alec-testdata/src && echo "fn main() {}" > alec-testdata/src/lib.rs && \
    mkdir -p alec-exporter/src && echo "fn main() {}" > alec-exporter/src/main.rs

# Build dependencies only (cached layer)
RUN cargo build --release --package alec-exporter 2>/dev/null || true

# Copy actual source code
COPY alec-codec/src alec-codec/src
COPY alec-gateway/src alec-gateway/src
COPY alec-complexity/src alec-complexity/src
COPY alec-testdata/src alec-testdata/src
COPY alec-exporter/src alec-exporter/src

# Touch files to invalidate cached builds
RUN touch alec-codec/src/lib.rs alec-gateway/src/lib.rs \
    alec-complexity/src/lib.rs alec-testdata/src/lib.rs \
    alec-exporter/src/main.rs

# Build release binary
RUN cargo build --release --package alec-exporter

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd --create-home --user-group alec

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/alec-exporter /app/alec-exporter

# Copy sample datasets (optional, can be mounted)
COPY alec-testdata/datasets /app/datasets

# Set ownership
RUN chown -R alec:alec /app

USER alec

# Expose metrics port
EXPOSE 9100

# Default command
ENTRYPOINT ["/app/alec-exporter"]
CMD ["--port", "9100"]

# Labels
LABEL org.opencontainers.image.title="ALEC Exporter"
LABEL org.opencontainers.image.description="Prometheus exporter for ALEC metrics"
LABEL org.opencontainers.image.source="https://github.com/zeekmartin/alec-codec"
LABEL org.opencontainers.image.vendor="David Martin Venti"
LABEL org.opencontainers.image.licenses="AGPL-3.0 OR Commercial"
