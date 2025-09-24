FROM rust:1.90.0-slim-bookworm as builder

WORKDIR /usr/src/app
COPY . .

RUN apt update && apt install pkg-config libssl-dev -y
RUN cargo build --release

RUN cp target/release/proposal-watcher /proposal-watcher

FROM rust:1.90.0-slim-bookworm
WORKDIR /usr/src/app
COPY --from=builder /proposal-watcher /usr/bin/proposal-watcher
COPY ./chains.toml ./chains.toml

CMD ["/usr/bin/proposal-watcher", "start", "-c", "/usr/src/app/chains.toml"]