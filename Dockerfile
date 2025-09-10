# Single stage build for debugging
FROM rust:1.89-alpine AS builder

# Install build dependencies including static libraries
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig ca-certificates

# Create app directory
WORKDIR /usr/src/app

# Copy source files
COPY Cargo.toml ./
COPY src ./src

# Build for release
RUN cargo build --release

FROM alpine:3.22

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

# Set logging level for tracing
ENV RUST_BACKTRACE=1
ENV RUST_LOG=info

# Expose port 80
EXPOSE 80

# Run the binary
ENTRYPOINT ["/usr/local/bin/geoip-api"]
CMD ["--bind", "0.0.0.0:80", "--data-dir", "/data"]