FROM rust:1.76 AS builder
RUN rustup target add x86_64-unknown-linux-musl
COPY . /opt/xc-bot/
RUN apt-get update && apt-get install -y --no-install-recommends musl-tools
RUN cd /opt/xc-bot \
 && cargo build --release --target x86_64-unknown-linux-musl

FROM alpine:3.13
RUN apk update && apk add dumb-init
RUN mkdir /xc-bot/ && chmod 0700 /xc-bot/

VOLUME [ "/xc-bot" ]
COPY --from=builder /opt/xc-bot/target/x86_64-unknown-linux-musl/release/xc-bot /usr/local/bin/xc-bot
RUN mkdir /etc/xc-bot/

WORKDIR /xc-bot

# Note: Use dumb-init in order to fulfil our PID 1 responsibilities,
# see https://github.com/Yelp/dumb-init
ENTRYPOINT [ "/usr/bin/dumb-init", "--" ]
CMD [ "xc-bot", "--config", "/etc/xc-bot/config.toml" ]
