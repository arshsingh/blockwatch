FROM rust:1.72.0-alpine as builder

WORKDIR /blockwatch

RUN apk update && apk add --no-cache musl-dev

RUN cargo init --vcs none
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release

COPY . .
RUN touch src/main.rs && cargo build --release

FROM alpine

WORKDIR /blockwatch
COPY --from=builder /blockwatch/target/release/blockwatch /bin

CMD ["blockwatch"]
