# 旅途计划

多人旅行行程规划 + 智能记账分账。后端 Axum + PostgreSQL，前端微信原生小程序。

## 目录

- 后端：`server/`
- 小程序：`miniprogram/`
- 部署：根目录 `docker-compose.yml`

---

## Docker 部署（推荐）

### 1. 准备环境变量

```bash
cp .env.example .env
```

编辑 `.env`，至少修改：

- `DATABASE_URL`（复用已有 PostgreSQL，例如 `postgres://user:pass@host:5432/travel`）
- `JWT_SECRET`
- `WECHAT_APPID` / `WECHAT_SECRET`（小程序正式登录）

默认**不**内置数据库；API 只读 `DATABASE_URL` 连接你已有的 PostgreSQL。

基础镜像默认走个人阿里云仓库（与 debian 同前缀）：`rust` / `nginx`；可用 `.env` 的 `RUST_IMAGE` / `NGINX_IMAGE` 覆盖。构建前需已 `docker login` 该仓库，并确保仓库中有对应镜像 tag。

### 2. 启动

```bash
docker compose up -d --build
```

- 对外入口：`gateway`，只映射一个端口 `PORT`（默认 **8080**）
- 外层 Nginx 把 `travel.jiangker.cn` 端口转发到本机该端口即可；`/api`、`/admin` 在容器内分流
- API 只在 Docker 内网监听，不再单独对外开端口
- 健康检查：`GET /health`
- 数据库迁移在 API 启动时自动执行

**同域路径（gateway 内）**

| 路径 | 用途 |
|------|------|
| `/api/*` | 小程序 / 业务 API |
| `/health` | 健康检查 |
| `/admin/*` | 后续管理端（未部署时 502/503，属正常） |

常用命令：

```bash
docker compose ps
docker compose logs -f gateway api
docker compose down          # 停服务
```

### 3. 小程序侧

1. `miniprogram/utils/config.js`：开发版走本机，体验版/正式版走 `https://travel.jiangker.cn`（请求仍是 `/api/...`）
2. 小程序后台将 request 合法域名设为 `travel.jiangker.cn`
3. 微信开发者工具打开 `miniprogram/`，构建 npm 后上传审核

生产环境请保持 `DEV_MODE=0`（compose 默认已是 0）。

---

## 本地开发

后端：

```bash
cd server
cp .env.example .env   # 按需修改 DATABASE_URL
cargo run
```

默认 API：`http://127.0.0.1:3000`

小程序：

```bash
cd miniprogram
npm install
```

用微信开发者工具打开 `miniprogram/`，构建 npm；真机调试时把 `baseUrl` 改成电脑局域网 IP。

新用户无真实行程时会自动出现「川西小环线」已结束示例，可查看行程、账单与分账。

---

## 已覆盖能力

- 微信登录、多旅途列表、创建/加入/退出/归档
- 行程点位、排序、路书地图
- 记账分摊、账单池 / 我的花销、智能分账（含团体汇总）
