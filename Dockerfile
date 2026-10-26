# Build the SPA, then the server, then ship both on a minimal base.
#
# Base images come from Google's Docker Hub mirror: Docker Hub rate-limits
# anonymous pulls and public.ecr.aws throttles hard.

# --- Frontend: Vite build, served from the server's static directory -------
FROM mirror.gcr.io/library/node:22-alpine AS frontend
WORKDIR /app/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# --- Backend: a static Rust binary -----------------------------------------
FROM mirror.gcr.io/library/rust:1-slim AS backend
WORKDIR /src
# rustls' crypto provider (ring) compiles C and assembly at build time.
RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential perl nasm \
    && rm -rf /var/lib/apt/lists/*

# Dependencies in a layer of their own, so a code change rebuilds only the bin.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release --locked \
    && rm -rf src

COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs && cargo build --release --locked

# --- Runtime: glibc and CA certificates, no shell --------------------------
FROM gcr.io/distroless/cc-debian12
COPY --from=backend /src/target/release/esker /app/esker
COPY --from=frontend /app/web/dist /app/static
ENV BOARD_STATIC_DIR=/app/static \
    BOARD_DATA_DIR=/data
VOLUME /data
EXPOSE 8080
ENTRYPOINT ["/app/esker"]
