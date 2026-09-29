// 联调说明：
// - 线上 / 真机 / 开发工具连已部署服务：https://travel.jiangker.cn
// - 本机 cargo run：改成 http://127.0.0.1:3000
// - 直连服务器 gateway（外层 Nginx 未好时）：http://服务器IP:8080
module.exports = {
  baseUrl: 'https://travel.jiangker.cn',
  // 跟代码包走。提审包改成新版本号，并在服务器 REVIEW_VERSION 填同一个号。
  // 审核通过后把 REVIEW_VERSION 清空，这个版本也会恢复邀请码。不要和当前线上版本用同一个号。
  version: '1.0.3',
  mapKey: '',
  mapSk: '',
}
