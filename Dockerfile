FROM rust:1.69 AS builder

WORKDIR /usr/src/app
COPY ./protohackers .
# Build and cache the binary and dependent crates in release mode.
RUN --mount=type=cache,target=/usr/local/cargo,from=rust:latest,source=/usr/local/cargo \
    --mount=type=cache,target=target \
    cargo build --release && mv ./target/release/protohackers ./protohackers

FROM debian:bullseye-slim
RUN useradd -ms /bin/bash app

USER app
WORKDIR /app

COPY --from=builder /usr/src/app/protohackers /app/protohackers

ENV RUST_LOG="protohackers=trace"
ENTRYPOINT ["./protohackers"]
