# Setup build image for multistage build
FROM rust:latest as builder
# install build deps
RUN apt-get update && apt-get upgrade -y
RUN apt-get install -yqq --no-install-recommends capnproto build-essential cmake clang libclang-dev libgsasl7-dev

WORKDIR /usr/src/bffh
COPY . .
RUN cargo install --path .


# Setup deployable image
FROM debian:buster-slim
# Install runtime deps
RUN apt-get update && apt-get upgrade -yqq
RUN apt-get install -yqq libgsasl7 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/diflouroborane /usr/local/bin/diflouroborane
# RUN diflouroborane --print-default > /etc/diflouroborane.toml
VOLUME /etc/diflouroborane.toml
EXPOSE 59661
ENTRYPOINT ["diflouroborane"]