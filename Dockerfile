FROM rust:1.69 AS builder

WORKDIR /usr/src/app
COPY ./protohackers .
RUN cargo build --release && mv ./target/release/protohackers ./protohackers

FROM debian:bullseye-slim
RUN useradd -ms /bin/bash app

USER app
WORKDIR /app

COPY --from=builder /usr/src/app/protohackers /app/protohackers

ENV RUST_LOG="protohackers=trace"
ENTRYPOINT ["./protohackers"]
