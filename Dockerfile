FROM --platform=$BUILDPLATFORM alpine:latest as copy
ARG TARGETPLATFORM
RUN case "$TARGETPLATFORM" in \
  "linux/arm/v7") echo armv7-unknown-linux-gnueabihf > /rust_target.txt ;; \ 
  "linux/arm/v6") echo arm-unknown-linux-gnueabihf > /rust_target.txt ;; \ 
  "linux/arm64") echo aarch64-unknown-linux-gnu > /rust_target.txt ;; \
  "linux/amd64") echo x86_64-unknown-linux-gnu > /rust_target.txt ;; \  
  *) exit 1 ;; \
esac

WORKDIR /usr/src/bffh
COPY . .
RUN cp target/$(cat /rust_target.txt)/release/bffhd ./bffhd.bin

# Setup deployable image
FROM ubuntu:22.04
RUN apt-get update && apt-get upgrade -y
RUN apt-get install -yqq --no-install-recommends python3 python3-pip
RUN pip3 install paho-mqtt
COPY --from=copy /usr/src/bffh/bffhd.bin /usr/local/bin/bffhd
VOLUME /etc/bffh/
VOLUME /var/lib/bffh/
VOLUME /usr/local/lib/bffh/adapters/
EXPOSE 59661
ENTRYPOINT ["sh", "-c", "bffhd -c /etc/bffh/bffh.dhall --load=/etc/bffh/users.toml; bffhd -c /etc/bffh/bffh.dhall"]
