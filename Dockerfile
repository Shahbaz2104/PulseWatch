# syntax=docker/dockerfile:1
# Full-stack image: builds the Vite frontend, compiles the Rust backend, and
# serves both from one process (STATIC_DIR + SPA fallback in the API router).

# ---------- frontend build ----------
FROM node:22-bookworm-slim AS frontend
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

# ---------- backend build ----------
FROM rust:1-bookworm AS backend
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY backend ./backend
COPY migrations ./migrations

RUN cargo build --release --manifest-path backend/Cargo.toml \
    && cp target/release/pulsewatch /usr/local/bin/pulsewatch

# ---------- runtime ----------
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=backend /usr/local/bin/pulsewatch /usr/local/bin/pulsewatch
COPY --from=frontend /app/frontend/dist /app/static
COPY migrations /app/migrations

ENV HOST=0.0.0.0 \
    PORT=3000 \
    DATABASE_URL=sqlite:////data/pulsewatch.db \
    STATIC_DIR=/app/static \
    RUST_LOG=info

VOLUME ["/data"]
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=3s --start-period=15s --retries=3 \
    CMD ["bash", "-c", "exec 3<>/dev/tcp/127.0.0.1/3000 && printf 'GET /health HTTP/1.0\\r\\n\\r\\n' >&3 && head -1 <&3 | grep -q 200"]

ENTRYPOINT ["/usr/local/bin/pulsewatch"]
