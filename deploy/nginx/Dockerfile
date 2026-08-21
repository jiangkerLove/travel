ARG BASE_IMAGE=crpi-jya41iij3cykq66t.cn-chengdu.personal.cr.aliyuncs.com/jiangker_love/debian:stable-20240904-slim-shanghai
FROM ${BASE_IMAGE}

RUN set -eux; \
  if [ -f /etc/apt/sources.list.d/debian.sources ]; then \
    sed -i \
      -e 's|https\?://deb.debian.org|https://mirrors.aliyun.com|g' \
      -e 's|https\?://security.debian.org|https://mirrors.aliyun.com|g' \
      /etc/apt/sources.list.d/debian.sources; \
  fi; \
  if [ -f /etc/apt/sources.list ]; then \
    sed -i \
      -e 's|deb.debian.org|mirrors.aliyun.com|g' \
      -e 's|security.debian.org|mirrors.aliyun.com|g' \
      /etc/apt/sources.list; \
  fi; \
  apt-get update; \
  apt-get install -y --no-install-recommends nginx; \
  rm -rf /var/lib/apt/lists/*; \
  rm -f /etc/nginx/sites-enabled/default; \
  mkdir -p /var/log/nginx /var/lib/nginx /var/run; \
  ln -sf /dev/stdout /var/log/nginx/access.log; \
  ln -sf /dev/stderr /var/log/nginx/error.log

COPY nginx.conf /etc/nginx/nginx.conf

EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
