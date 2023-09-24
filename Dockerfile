FROM rust:1.72.0-alpine as builder

WORKDIR /blockwatch

RUN apk update && apk add --no-cache musl-dev

COPY Cargo.toml Cargo.toml

RUN mkdir -p src/ && \
  echo "fn main() {}" > src/main.rs && \
  cargo build --release && \
  rm -f target/release/deps/blockwatch*

COPY . .

RUN cargo build --locked --release

FROM scratch

WORKDIR /blockwatch

COPY --from=builder /blockwatch/target/release/blockwatch .

CMD ["./blockwatch"]
