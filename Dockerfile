FROM rust:1.67 as build
RUN USER=root cargo new --bin voyage-abeon
WORKDIR /voyage-abeon

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# Build only dependencies - we can cache them but not the actual cosw
RUN cargo build --release
RUN rm src/*.rs 

# now copy our code
COPY ./src ./src

# build it all
RUN rm ./target/release/deps/holodeck*
RUN cargo build --release -C target-feature=+crt-static

FROM debian:buster-slim

COPY --from=build /voyage-abeon/target/release/voyage-abeon .
COPY ./assets ./assets

CMD ["./voyage-abeon"]