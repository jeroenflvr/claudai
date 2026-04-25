#  Stage 1: build 
FROM rust:1.95-bookworm AS builder

RUN apt-get update && apt-get install -y \
    cmake \
    g++ \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Cache dependencies — copy manifests first, then source
COPY Cargo.toml Cargo.lock ./
# Dummy main so `cargo build` can resolve and cache all deps
RUN mkdir src && echo 'fn main(){}' > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Now build the real source
COPY src ./src
COPY templates ./templates
# Touch main.rs so cargo knows it changed
RUN touch src/main.rs && cargo build --release

#  Stage 2: runtime 
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libstdc++6 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /build/target/release/claudia ./claudia

# Data directory for the DuckDB file
RUN mkdir -p /data

EXPOSE 3000

ENV DB_PATH=/data/claudia.duckdb \
    PORT=3000

ENTRYPOINT ["./claudia"]
