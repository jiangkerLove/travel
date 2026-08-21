# 旅途计划 API · 对齐 sesame-rise：国内源 Debian 编译，运行时不再 apt-get
ARG BASE_IMAGE=crpi-jya41iij3cykq66t.cn-chengdu.personal.cr.aliyuncs.com/jiangker_love/debian:stable-20240904-slim-shanghai
# 腾讯云机器可改为 http://mirrors.tencentyun.com
ARG APT_MIRROR=http://mirrors.aliyun.com

FROM ${BASE_IMAGE} AS builder
ARG APT_MIRROR

RUN set -eux; \
  . /etc/os-release; \
  rm -f /etc/apt/sources.list.d/*.sources; \
  printf '%s\n' \
    "deb ${APT_MIRROR}/debian ${VERSION_CODENAME} main contrib non-free non-free-firmware" \
    "deb ${APT_MIRROR}/debian-security ${VERSION_CODENAME}-security main contrib non-free non-free-firmware" \
    "deb ${APT_MIRROR}/debian ${VERSION_CODENAME}-updates main contrib non-free non-free-firmware" \
    > /etc/apt/sources.list; \
  printf 'Acquire::Retries "5";\n' > /etc/apt/apt.conf.d/80retries; \
  apt-get update; \
  apt-get install -y --no-install-recommends \
    ca-certificates curl pkg-config gcc libc6-dev busybox-static; \
  rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUSTUP_DIST_SERVER=https://rsproxy.cn \
    RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup

RUN curl --proto '=https' --tlsv1.2 -sSf https://rsproxy.cn/rustup-init.sh \
    | sh -s -- -y --default-toolchain stable --profile minimal \
  && mkdir -p "${CARGO_HOME}" \
  && printf '%s\n' \
    '[source.crates-io]' \
    'replace-with = "rsproxy-sparse"' \
    '' \
    '[source.rsproxy]' \
    'registry = "https://rsproxy.cn/crates.io-index"' \
    '' \
    '[source.rsproxy-sparse]' \
    'registry = "sparse+https://rsproxy.cn/index/"' \
    '' \
    '[registries.rsproxy]' \
    'index = "https://rsproxy.cn/crates.io-index"' \
    '' \
    '[net]' \
    'git-fetch-with-cli = true' \
    > "${CARGO_HOME}/config.toml"

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY src ./src
RUN cargo build --release \
  && strip target/release/server

FROM ${BASE_IMAGE} AS runtime

# 运行阶段不再 apt-get：证书和 wget 从 builder 拷过来
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder /bin/busybox /usr/local/bin/busybox
RUN ln -sf /usr/local/bin/busybox /usr/local/bin/wget \
  && useradd -r -u 10001 -m appuser

WORKDIR /app
COPY --from=builder /app/target/release/server /usr/local/bin/travel-api

USER appuser
ENV PORT=3000 \
    RUST_LOG=server=info,sqlx=warn,tower_http=info \
    DEV_MODE=0 \
    SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt

EXPOSE 3000
HEALTHCHECK --interval=10s --timeout=5s --start-period=20s --retries=12 \
  CMD wget -qO- http://127.0.0.1:3000/health || exit 1

CMD ["travel-api"]
