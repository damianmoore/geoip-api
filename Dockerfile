# Single stage build for debugging
FROM rust:1.89-alpine

# Install build dependencies including static libraries
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig ca-certificates

# Create app directory
WORKDIR /usr/src/app

# Copy source files
COPY Cargo.toml ./
COPY src ./src

# Build for release
RUN cargo build --release

# Create a non-root user
RUN adduser -D -u 1000 appuser

# Create data directory
RUN mkdir -p /data && chown appuser:appuser /data

# Switch to non-root user
USER appuser

# Set logging level for tracing
ENV RUST_BACKTRACE=1
ENV RUST_LOG=info

# Expose port 80
EXPOSE 80

# Run the binary
ENTRYPOINT ["/usr/src/app/target/release/geoip-api"]
CMD ["--bind", "0.0.0.0:80", "--data-dir", "/data"]