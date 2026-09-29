// 联调说明：
// - 线上 / 真机 / 开发工具连已部署服务：https://travel.jiangker.cn
// - 本机 cargo run：改成 http://127.0.0.1:3000
// - 直连服务器 gateway（外层 Nginx 未好时）：http://服务器IP:8080
module.exports = {
  baseUrl: 'https://travel.jiangker.cn',
  // 跟代码包走。提审前若接口有不兼容改动，先改这里，线上旧包和审核包才能被服务端区分。
  version: '1.0.2',
  mapKey: '',
  mapSk: '',
}
