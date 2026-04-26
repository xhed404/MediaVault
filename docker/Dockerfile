FROM node:22-bookworm-slim AS web_builder
WORKDIR /app
COPY apps/web/package.json apps/web/package-lock.json* apps/web/ ./
RUN npm install
COPY apps/web/ ./
RUN npm run build

FROM rust:1.88-bookworm AS server_builder
WORKDIR /app
COPY Cargo.toml ./
COPY apps/server/Cargo.toml apps/server/Cargo.toml
RUN mkdir -p apps/server/src && echo "fn main(){}" > apps/server/src/main.rs
RUN cargo build -p mediavault_server --release
COPY apps/server/ apps/server/
RUN cargo build -p mediavault_server --release

FROM debian:bookworm-slim
RUN useradd -m -u 10001 appuser
WORKDIR /app
COPY --from=server_builder /app/target/release/mediavault_server /app/mediavault_server
COPY --from=web_builder /app/dist /app/web_dist
ENV BIND_ADDR=0.0.0.0:8080
ENV DATABASE_URL=sqlite:/data/app.sqlite
ENV STORAGE_ROOT=/data/objects
ENV WEB_DIST=/app/web_dist
USER appuser
EXPOSE 8080
CMD ["/app/mediavault_server"]

