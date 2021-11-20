FROM docker.io/rust:slim-bullseye as builder

WORKDIR /build

RUN apt-get update
RUN apt-get install --assume-yes libmariadb-dev-compat ca-certificates libssl-dev pkg-config

# Avoid having to install/build all dependencies by copying
# the Cargo files and making a dummy src/main.rs
COPY Cargo.toml .
RUN mkdir src
RUN echo "fn main() {}" > src/main.rs
RUN cargo build --release

COPY src migrations ./

# We need to touch our real main.rs file or else docker will use
# the cached one.
RUN touch src/main.rs

RUN cargo build --release

FROM docker.io/debian:bullseye-slim

RUN apt-get update
RUN apt-get install --assume-yes libmariadb-dev-compat ca-certificates openssl

WORKDIR /app

COPY --from=builder /build/target/release/foobot2 .
COPY static templates Rocket.toml ./

CMD ["/app/foobot2"]
