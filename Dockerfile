FROM docker.io/rust:slim-bullseye as builder

RUN apt-get update
RUN apt-get install --assume-yes libmariadb-dev-compat pkg-config git

RUN cargo install sccache

ENV RUSTC_WRAPPER="/usr/local/cargo/bin/sccache"
ARG SCCACHE_MEMCACHED

WORKDIR /build

# Avoid having to install/build all dependencies by copying
# the Cargo files and making a dummy src/main.rs
COPY Cargo.lock Cargo.toml ./
RUN mkdir src
RUN echo "fn main() {}" > src/main.rs
RUN cargo build --release

RUN rustup component add rustfmt

# We need to touch our real main.rs file or else docker will use
# the cached one.
COPY . .
RUN touch src/main.rs


RUN cargo build --release

FROM docker.io/debian:bullseye-slim

RUN apt-get update
RUN apt-get install --assume-yes libmariadb-dev-compat ca-certificates openssl

WORKDIR /app

COPY --from=builder /build/target/release/foobot2 .
COPY static ./static
COPY templates ./templates
COPY Rocket.toml .

STOPSIGNAL SIGINT

CMD ["/app/foobot2"]
