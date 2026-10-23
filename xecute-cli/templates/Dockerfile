# syntax=docker/dockerfile:1
# Build tools stay in this stage; the final image receives only the stripped binary.
FROM rust:1.98.1-alpine AS build
ARG TARGETARCH
# Optional: docker build --build-arg CARGO_FEATURES=s3 ...
ARG CARGO_FEATURES=""
RUN apk add --no-cache gcc musl-dev \
    && if [ -n "$CARGO_FEATURES" ]; then apk add --no-cache cmake make perl g++; fi
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY public ./public
COPY AppConfig.toml ./
# Adapt the generated loopback default for container networking; preserve custom binds.
RUN sed -i 's/^bind_address = "127\.0\.0\.1:3000"$/bind_address = "0.0.0.0:3000"/' AppConfig.toml \
    && mkdir -p /app/logs /app/storage/uploads
# BuildKit selects the matching static musl target for the requested image architecture.
RUN case "$TARGETARCH" in \
      amd64) target=x86_64-unknown-linux-musl ;; \
      arm64) target=aarch64-unknown-linux-musl ;; \
      *) echo "Unsupported architecture: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && rustup target add "$target" \
    && if [ -n "$CARGO_FEATURES" ]; then \
         cargo build --release --locked --target "$target" --features "$CARGO_FEATURES"; \
       else cargo build --release --locked --target "$target"; fi \
    && cp "target/$target/release/__PROJECT_NAME__" /app/server

# Minimal non-root runtime with certificate data, without a shell or package manager.
# Configure /health/live and /health/ready probes in your orchestrator.
FROM gcr.io/distroless/static-debian13:nonroot AS runtime
WORKDIR /app
COPY --from=build --chown=65532:65532 /app/server /app/server
COPY --from=build --chown=65532:65532 /app/public /app/public
COPY --from=build --chown=65532:65532 /app/AppConfig.toml /app/AppConfig.toml
COPY --from=build --chown=65532:65532 /app/logs /app/logs
COPY --from=build --chown=65532:65532 /app/storage /app/storage
USER 65532:65532
EXPOSE 3000
STOPSIGNAL SIGTERM
ENTRYPOINT ["/app/server"]
