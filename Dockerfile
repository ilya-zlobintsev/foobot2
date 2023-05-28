FROM node:latest as frontend
WORKDIR /web
COPY web .
RUN npm install
RUN npm run build

FROM docker.io/rust:slim-bullseye as builder

RUN apt-get update
RUN apt-get install --assume-yes libmariadb-dev-compat pkg-config git protobuf-compiler

WORKDIR /build

# Avoid having to install/build all dependencies by copying
# the Cargo files and making a dummy src/main.rs
COPY Cargo.lock Cargo.toml ./
RUN mkdir src
RUN echo "fn main() {}" > src/main.rs
RUN cargo build --release

RUN rustup component add rustfmt

COPY . .
RUN touch src/main.rs

RUN cargo build --release

FROM docker.io/debian:bullseye-slim

RUN apt-get update
RUN apt-get install --assume-yes libmariadb-dev-compat ca-certificates openssl git

WORKDIR /app

COPY --from=builder /build/target/release/foobot2 .
COPY --from=frontend /web/dist ./web/dist

STOPSIGNAL SIGINT

CMD ["/app/foobot2"]
