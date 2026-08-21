module.exports = {
  // 同域：正式环境 https://travel.jiangker.cn ，接口前缀仍为 /api/*
  // develop：本地；trial / release：线上
  baseUrl: (() => {
    try {
      const env = wx.getAccountInfoSync().miniProgram.envVersion
      if (env === 'develop') return 'http://127.0.0.1:3000'
    } catch (_) {}
    return 'https://travel.jiangker.cn'
  })(),
  mapKey: '',
  mapSk: '',
}
