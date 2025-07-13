# Production-Ready Secure Multi-stage Build
FROM cgr.dev/chainguard/rust:latest as builder

# Install minimal build dependencies
RUN apk add --no-cache \
    pkgconfig \
    openssl-dev \
    postgresql-dev

# Create app directory
WORKDIR /app

# Copy dependency files first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Create dummy main.rs to build dependencies only
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source code
COPY src ./src
COPY migrations ./migrations

# Set environment variables for static linking
ENV PKG_CONFIG_ALLOW_CROSS=1
ENV OPENSSL_STATIC=true

# Build the actual application
RUN cargo build --release

# Runtime stage - Use Google's distroless image for minimal attack surface
FROM gcr.io/distroless/static-debian12:nonroot

# Copy the built application from the correct path
COPY --from=builder /app/target/release/shopify-oauth-rust /app/shopify-oauth-rust

# Copy migrations
COPY --from=builder /app/migrations /app/migrations

# Copy CA certificates for HTTPS
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Set working directory
WORKDIR /app

# Expose port
EXPOSE 3000

# Set environment variables
ENV RUST_LOG=info
ENV ENVIRONMENT=production
ENV DATABASE_URL=""
ENV SHOPIFY_API_KEY=""
ENV SHOPIFY_API_SECRET=""
ENV ENCRYPTION_KEY=""

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["./shopify-oauth-rust", "--help"]

# Run the application
ENTRYPOINT ["./shopify-oauth-rust"]
