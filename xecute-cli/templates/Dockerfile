# syntax=docker/dockerfile:1
# Build tools stay in this stage; the final image receives only the stripped binary.
FROM rust:1.98.1-alpine AS build
ARG TARGETARCH
RUN apk add --no-cache gcc musl-dev
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY public ./public
# BuildKit selects the matching static musl target for the requested image architecture.
RUN case "$TARGETARCH" in \
      amd64) target=x86_64-unknown-linux-musl ;; \
      arm64) target=aarch64-unknown-linux-musl ;; \
      *) echo "Unsupported architecture: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && rustup target add "$target" \
    && cargo build --release --locked --target "$target" \
    && cp "target/$target/release/__PROJECT_NAME__" /app/server

# Minimal non-root runtime with certificate data, without a shell or package manager.
# Configure /health/live and /health/ready probes in your orchestrator.
FROM gcr.io/distroless/static-debian13:nonroot AS runtime
WORKDIR /app
COPY --from=build --chown=65532:65532 /app/server /app/server
COPY --from=build --chown=65532:65532 /app/public /app/public
USER 65532:65532
ENV BIND_ADDRESS=0.0.0.0:3000 RUST_LOG=info
EXPOSE 3000
STOPSIGNAL SIGTERM
ENTRYPOINT ["/app/server"]
