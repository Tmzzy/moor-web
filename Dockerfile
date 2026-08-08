# syntax=docker/dockerfile:1.7

FROM node:24-bookworm-slim AS frontend

WORKDIR /build
RUN corepack enable
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY packages/types/package.json packages/types/package.json
RUN --mount=type=cache,id=pnpm-store,target=/pnpm/store,sharing=locked \
    pnpm config set store-dir /pnpm/store && \
    pnpm install --frozen-lockfile
COPY . .
RUN pnpm build:frontend

FROM rust:1.88-bookworm AS backend

WORKDIR /build/backend
ARG TARGETARCH
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=cargo-target-${TARGETARCH},target=/build/backend/target,sharing=locked \
    cargo build --locked --release && \
    cp target/release/moor-server /tmp/moor-server

FROM debian:bookworm-slim AS runtime

LABEL org.opencontainers.image.title="Moor" \
      org.opencontainers.image.description="Web-based MCP gateway manager" \
      org.opencontainers.image.source="https://github.com/Tmzzy/moor-web" \
      org.opencontainers.image.licenses="Apache-2.0"

ENV MOOR_HOST=0.0.0.0 \
    MOOR_PORT=9223 \
    MOOR_DATA_DIR=/data \
    MOOR_STATIC_DIR=/app/dist \
    MOOR_USERNAME=moor

WORKDIR /app
RUN apt-get update && \
    apt-get install --yes --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd --gid 10001 moor && \
    useradd --no-log-init --uid 10001 --gid moor --home-dir /nonexistent --shell /usr/sbin/nologin moor && \
    mkdir -p /data && \
    chown moor:moor /data

COPY --from=frontend /build/dist /app/dist
COPY --from=backend /tmp/moor-server /app/moor-server
COPY LICENSE NOTICE /app/licenses/

USER 10001:10001
EXPOSE 9223
VOLUME ["/data"]
STOPSIGNAL SIGTERM

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD ["/app/moor-server", "--healthcheck"]

ENTRYPOINT ["/app/moor-server"]
