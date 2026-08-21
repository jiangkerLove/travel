# 统一基础镜像（个人阿里云）
ARG BASE_IMAGE=crpi-jya41iij3cykq66t.cn-chengdu.personal.cr.aliyuncs.com/jiangker_love/debian:stable-20240904-slim-shanghai

# ---- build ----
FROM ${BASE_IMAGE} AS builder
WORKDIR /app

RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    ca-certificates curl build-essential pkg-config libssl-dev \
  && rm -rf /var/lib/apt/lists/*

# 安装 Rust（国内静态资源镜像）
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static \
    RUSTUP_UPDATE_ROOT=https://mirrors.ustc.edu.cn/rust-static/rustup

RUN curl --proto '=https' --tlsv1.2 -sSf https://rsproxy.cn/rustup-init.sh | sh -s -- -y --default-toolchain stable \
  && rustc --version

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

COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
RUN mkdir src && echo "fn main() {}" > src/main.rs \
  && cargo build --release \
  && rm -rf src

COPY src ./src
RUN touch src/main.rs && cargo build --release \
  && strip target/release/server

# ---- runtime ----
FROM ${BASE_IMAGE} AS runtime
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
