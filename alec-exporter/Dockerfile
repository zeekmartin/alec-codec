# ALEC Exporter Dockerfile
FROM rust:1.83-bookworm AS builder
WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY benches ./benches
COPY alec-ffi alec-ffi
COPY alec-gateway alec-gateway
COPY alec-complexity alec-complexity
COPY alec-testdata alec-testdata
COPY alec-exporter alec-exporter

# Build release binary
RUN cargo build --release --package alec-exporter

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --user-group alec
WORKDIR /app

COPY --from=builder /app/target/release/alec-exporter /app/alec-exporter

USER alec
EXPOSE 9100

ENTRYPOINT ["/app/alec-exporter"]
CMD ["--port", "9100"]
