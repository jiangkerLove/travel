# 结伴出行 · 初版

多人旅行行程规划 + 智能记账分账。后端 Axum + PostgreSQL，前端微信原生小程序 + TDesign。

## 目录

- 后端：`C:\Users\MSI\project\travel\server`
- 小程序：`C:\Users\MSI\WeChatProjects\miniprogram-17`（微信开发者工具打开此目录）

## 1. 启动 PostgreSQL

本机需有 PostgreSQL。可用 Docker：

```bash
docker compose up -d
```

或手动创建数据库：

```sql
CREATE USER travel WITH PASSWORD 'travel';
CREATE DATABASE travel OWNER travel;
```

默认连接：`postgres://travel:travel@127.0.0.1:5432/travel`

## 2. 启动后端

```bash
cd C:\Users\MSI\project\travel\server
cargo run
```

服务地址 `http://127.0.0.1:3000`，启动时自动执行 migrations。

微信登录：在 `server/.env` 填写 `WECHAT_APPID` / `WECHAT_SECRET`。不填则走开发登录（昵称或演示账号）。

地图按「路书」展示：编号点位 + 点到点连线。自驾/步行在额度允许时会按导航路线简化；飞机画航线，高铁/火车只用直线。路线会缓存，避免每次打开地图都打腾讯接口。额度用完或签名失败后自动停掉请求。点地点仍打开系统地图。

## 3. 打开小程序

1. 安装依赖并构建 npm（`miniprogram_npm` 不入库，需本地生成）：

```bash
cd C:\Users\MSI\project\travel\miniprogram
npm install
```

然后用微信开发者工具打开 `miniprogram` 目录，菜单 **工具 → 构建 npm**。

2. 详情 → 本地设置 → 不校验合法域名（已在项目配置关闭 `urlCheck`）
3. 真机调试时把 `utils/config.js` 的 `baseUrl` 改成电脑局域网 IP，例如 `http://192.168.1.8:3000`

## 4. 演示路径

新注册用户（还没有任何旅途）登录后，列表里会自动出现「川西小环线」示例攻略，可直接看行程、路书、账单和分账。体验完由团长归档即可。

开发环境也可在登录页：

1. 点「导入川西演示旅途」
2. 用「小鹿 / 阿伟 / 小林」进入
3. 邀请码 `DEMO88`

## 已覆盖能力

- 微信/演示登录、多旅途列表、创建/加入/退出/归档
- 五大分类行程点位、拖拽排序（上移下移）、绑定经纬度
- 全局地图 + 单日路线、点位筛选、交通方式差异化线条
- 集体/个人账单、隐私开关、点位绑定、平均分摊
- 花销统计、智能分账、团长锁定结算
