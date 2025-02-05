FROM rust:1.84.0-slim-bullseye AS build

WORKDIR /app

RUN apt update
RUN apt install -y build-essential pkg-config libssl-dev libsasl2-dev cmake

COPY ./Cargo.toml ./Cargo.toml
COPY . .

RUN cargo build --release

FROM debian:stable-slim

COPY --from=build /app/target/release/boros /usr/local/bin/boros

ENTRYPOINT [ "boros" ]
