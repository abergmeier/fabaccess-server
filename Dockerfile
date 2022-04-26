# Setup build image for multistage build
FROM rust:bullseye as builder
# install build deps
RUN apt-get update && apt-get upgrade -y
RUN apt-get install -yqq --no-install-recommends capnproto

WORKDIR /usr/src/bffh
COPY . .
RUN cargo build --release


# Setup deployable image
FROM debian:bullseye-slim
# Install runtime deps
#RUN apt-get update && apt-get upgrade -yqq
COPY --from=builder /usr/src/bffh/target/release/bffhd /usr/local/bin/bffhd
#COPY --from=builder /usr/src/bffh/examples/bffh.dhall /etc/diflouroborane.dhall
# RUN diflouroborane --print-default > /etc/diflouroborane.toml
VOLUME /etc/bffh/
VOLUME /var/lib/bffh/
VOLUME /usr/local/lib/bffh/adapters/
EXPOSE 59661
ENTRYPOINT ["sh", "-c", "bffhd -c /etc/bffh/bffh.dhall --load=/etc/bffh/users.toml; bffhd -c /etc/bffh/bffh.dhall"]
