# This is a multi-stage Dockerfile and requires >= Docker 17.05
# https://docs.docker.com/engine/userguide/eng-image/multistage-build/
FROM rust:1-slim AS builder

WORKDIR /build

# Resolve dependencies before copying source, so source-only changes don't invalidate the
# downloaded-dependencies layer.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

COPY . .
RUN touch src/main.rs && cargo build --release

FROM debian:trixie-slim

ARG USERNAME=sweetrpg
ARG BUILD_NUMBER=unset
ARG BUILD_JOB=unset
ARG BUILD_SHA=unset
ARG BUILD_DATE=unset
ARG BUILD_VERSION=unset

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --user-group --create-home --system --skel /dev/null $USERNAME

WORKDIR /app

RUN mkdir -p /app/config
COPY --from=builder /build/target/release/sweetrpg-game-systems-web /app/bin/sweetrpg-game-systems-web
COPY --from=builder /build/static /app/static

RUN echo "{\"number\":\"${BUILD_NUMBER}\",\"job\":\"${BUILD_JOB}\",\"sha\":\"${BUILD_SHA}\",\"date\":\"${BUILD_DATE}\",\"version\":\"${BUILD_VERSION}\"}" > /app/config/build-info.json \
    && chown -R ${USERNAME}:${USERNAME} /app

ENV PORT="8080"
ENV BUILD_INFO_PATH="/app/config/build-info.json"

EXPOSE 8080

USER ${USERNAME}

ENTRYPOINT ["/app/bin/sweetrpg-game-systems-web"]
