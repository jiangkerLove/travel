# ---- build ----
# 与运行时同属个人阿里云仓库；可用 --build-arg 覆盖
ARG RUST_IMAGE=crpi-jya41iij3cykq66t.cn-chengdu.personal.cr.aliyuncs.com/jiangker_love/rust:1-bookworm
FROM ${RUST_IMAGE} AS builder
WORKDIR /app

# crates.io 国内源，加快依赖下载
ENV CARGO_HOME=/usr/local/cargo
RUN mkdir -p ${CARGO_HOME} \
  && printf '%s\n' \
    '[source.crates-io]' \
    'replace-with = "rsproxy-sparse"' \
    '[source.rsproxy-sparse]' \
    'registry = "sparse+https://rsproxy.cn/index/"' \
    '[registries.rsproxy]' \
    'index = "https://rsproxy.cn/crates.io-index"' \
    '[net]' \
    'git-fetch-with-cli = true' \
    > ${CARGO_HOME}/config.toml

RUN apt-get update \
  && apt-get install -y --no-install-recommends pkg-config libssl-dev \
  && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
RUN mkdir src && echo "fn main() {}" > src/main.rs \
  && cargo build --release \
  && rm -rf src

COPY src ./src
RUN touch src/main.rs && cargo build --release \
  && strip target/release/server

# ---- runtime ----
FROM crpi-jya41iij3cykq66t.cn-chengdu.personal.cr.aliyuncs.com/jiangker_love/debian:stable-20240904-slim-shanghai AS runtime
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates libssl3 curl \
  && rm -rf /var/lib/apt/lists/* \
  && useradd --system --create-home --uid 10001 app

WORKDIR /app
COPY --from=builder /app/target/release/server /usr/local/bin/travel-api

USER app
ENV PORT=3000 \
    RUST_LOG=server=info,sqlx=warn,tower_http=info \
    DEV_MODE=0

EXPOSE 3000
HEALTHCHECK --interval=15s --timeout=5s --start-period=20s --retries=5 \
  CMD curl -fsS "http://127.0.0.1:${PORT}/health" >/dev/null || exit 1

CMD ["travel-api"]
