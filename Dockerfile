FROM docker.io/rust:1.53.0-slim-buster as builder

WORKDIR /build

RUN apt-get update
RUN apt-get install --assume-yes libmariadb-dev-compat libssl-dev pkg-config

# Avoid having to install/build all dependencies by copying
# the Cargo files and making a dummy src/main.rs
COPY Cargo.toml .
RUN mkdir src
RUN echo "fn main() {}" > src/main.rs
RUN cargo build --release

# We need to touch our real main.rs file or else docker will use
# the cached one.
COPY . .
RUN touch src/main.rs


RUN cargo build --release

FROM debian:buster-slim

RUN apt-get update
RUN apt-get install --assume-yes libmariadb-dev-compat

WORKDIR /app

COPY --from=builder /build/target/release/foobot2 .
COPY static ./static
COPY templates ./templates

CMD ["/app/foobot2"]
