# 旅途计划小程序 · 项目长期记忆

## 技术栈
- 微信小程序 + TDesign（t-button/t-cell/t-tabs 等），UI 主色为绿色系。
- 账号体系：`getApp().ensureLogin()`，旅途列表 `api.travelList(false)`，`decorate()` 在 index.js 派生 rangeText/countdown 等。
- 首页 `pages/index`、详情 `pages/travel/home`、记账 `pages/bill/edit`、结算 `pages/settle`、个人中心 `pages/profile`（含"工作余生"假期/退休计算）。

## UI 设计约定（重要 · 用户底线）
- **配色**：小清新薄荷绿系。主色 `#6cbfa6`、深 `#4f9f8a`、浅底 `#e9f6f0`、页面背景 `#f4f9f6`、ink `#2f3b35`、muted `#8fa19a`。令牌定义在 `app.wxss` 的 `page{}`，TDesign 品牌色阶也已同步为薄荷绿阶。
- **用户审美偏好**：要"小清新"但不要"游戏化/卡通"（emoji 缩略图被否决）；不要浓重渐变 hero（v2 emerald 被否）；不要删原版结构做减法（v3-v7 越删越难看）；"小清新"= 原版结构 + 柔和配色 + 极淡装饰，不是极简空。
- **硬底线（v12 确立）**：字段/数据/计算逻辑绝不能动；字段的**原有显示样式**也尽量别改（如 `.trip-count` 原版带绿色块背景，误删会让用户以为字段被改）。润色只做纯配色/纯阴影；新增装饰元素需先征得同意。
- **改样式前必做**：`git show HEAD:<file>` 确认该元素原版样式，避免误删原版特征。

## 工作流约定
- 改 UI 后必须用 Chrome headless 截图 `design-preview/redesign.html` 自审（预览文件曾与代码长期不同步，导致盲改）。
- Chrome headless 在本机需用：`--headless --disable-gpu --no-sandbox --no-zygote --single-process --disable-dev-shm-usage --disable-software-rasterizer --user-data-dir=/tmp/xxx`，否则被杀（137）。
- 预览 `design-preview/redesign.html` 必须与 `pages/index/*` 结构严格对齐（class 名、显示样式），否则预览骗人。

## 预览与产物
- `design-preview/redesign.html`：高保真预览（首页/详情/我的 三屏，底部 tab 可切换）。
- 截图存放：`.workbuddy/screenshots/`（内部验证用，不交付用户）。
