# 旅途计划

多人旅行行程规划 + 智能记账分账。后端 Axum + PostgreSQL，前端微信原生小程序。

## 目录

- 后端：`server/`
- 小程序：`miniprogram/`
- 部署：根目录 `docker-compose.yml`（风格对齐 sesame-rise，**不内置 HTTPS**）

---

## Docker 部署（推荐）

### 1. 准备环境变量

```bash
cp .env.example .env
```

编辑 `.env`，至少修改：

- `DATABASE_URL`：已有 Postgres；容器内用 `host.docker.internal`，不要写 `127.0.0.1`
- `JWT_SECRET`
- `WECHAT_APPID` / `WECHAT_SECRET`

### 2. 启动

```bash
docker compose up -d --build
```

- 对外只映射一个端口 `PORT`（默认 **8080** → gateway HTTP）
- 外层 Nginx 把 `travel.jiangker.cn` **端口转发**到该端口即可（HTTPS 在你外层处理）
- `api` 基于个人阿里云 `debian:…-shanghai` 构建；运行时不再 `apt-get`（证书 / wget 从 builder 拷入）
- `gateway` 基于现成 nginx 镜像构建（配置打进镜像，避免宿主机文件挂载踩坑）
- 启动时自动跑数据库迁移

**同域路径**

| 路径 | 用途 |
|------|------|
| `/api/*` | 业务 API |
| `/health` | 健康检查 |
| `/admin/*` | 后续管理端 |

```bash
docker compose ps
docker compose logs -f gateway api
docker compose down
```

### 3. 小程序侧

1. 体验版 / 正式版请求 `https://travel.jiangker.cn/api/...`
2. 小程序后台配置 request 合法域名 `travel.jiangker.cn`
3. 构建 npm 后上传审核

---

## 本地开发

```bash
cd server
cp .env.example .env   # 按需修改 DATABASE_URL
cargo run
```

默认 API：`http://127.0.0.1:3000`

```bash
cd miniprogram
npm install
```

用微信开发者工具打开 `miniprogram/` 并构建 npm。

---

## 已覆盖能力

- 微信登录、多旅途列表、创建/加入/退出/归档
- 行程点位、排序、路书地图
- 记账分摊、账单池 / 我的花销、智能分账（含团体汇总）
