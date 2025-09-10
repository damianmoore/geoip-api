# Build stage
FROM rust:1.89-alpine AS builder

# Install build dependencies including static libraries
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig

# Create app directory
WORKDIR /usr/src/app

# Copy manifests
COPY Cargo.toml ./

# Create dummy main.rs to cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy source code
COPY src ./src

# Build for release
RUN cargo build --release

# Runtime stage - using alpine for small size
FROM alpine:3.19

# Install ca-certificates for HTTPS requests
RUN apk add --no-cache ca-certificates

# Create a non-root user
RUN adduser -D -u 1000 appuser

# Create data directory
RUN mkdir -p /data && chown appuser:appuser /data

# Copy the binary from builder stage
COPY --from=builder /usr/src/app/target/release/geoip-api /usr/local/bin/geoip-api

# Switch to non-root user
USER appuser

# Expose port 80
EXPOSE 80

# Run the binary
ENTRYPOINT ["/usr/local/bin/geoip-api"]
CMD ["--bind", "0.0.0.0:80", "--data-dir", "/data"]
