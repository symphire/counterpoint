# ---------- Build stage ----------
FROM docker.io/library/rust:1.92.0-trixie AS builder

# Install CMake + native deps
RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependencies first
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy real source
COPY . .
RUN SQLX_OFFLINE=true cargo build --release

# ---------- Server runtime image ----------
FROM gcr.io/distroless/cc-debian13 AS server

WORKDIR /app

COPY ./certs/ /app/certs/
COPY ./settings/release.toml /app/settings/release.toml
COPY --from=builder /app/target/release/counterpoint /app/server

EXPOSE 8443
ENTRYPOINT ["/app/server"]

# ---------- Infra-demo runtime image ----------
FROM gcr.io/distroless/cc-debian13 AS infra-demo

WORKDIR /app

COPY --from=builder /app/target/release/infra_demo /app/infra_demo

ENTRYPOINT ["/app/infra_demo"]
