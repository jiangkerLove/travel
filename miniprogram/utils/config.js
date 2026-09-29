// 联调说明：
// - 线上 / 真机 / 开发工具连已部署服务：https://travel.jiangker.cn
// - 本机 cargo run：改成 http://127.0.0.1:3000
// - 直连服务器 gateway（外层 Nginx 未好时）：http://服务器IP:8080
module.exports = {
  baseUrl: 'https://travel.jiangker.cn',
  // 跟代码包走，请求头 X-App-Version 用的就是这个值。
  // 要和服务器 REVIEW_VERSION 完全一致才会隐藏邀请码。审核通过后清空 REVIEW_VERSION。
  version: '1.0.2',
  mapKey: '',
  mapSk: '',
}
