FROM rust:1-bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends musl-tools \
    && rustup target add x86_64-unknown-linux-musl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build \
    --release \
    --target x86_64-unknown-linux-musl

FROM scratch

COPY --from=builder \
    /app/target/x86_64-unknown-linux-musl/release/member-mgr \
    /member-mgr

ENTRYPOINT ["/member-mgr"]
