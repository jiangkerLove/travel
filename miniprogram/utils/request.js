const { baseUrl } = require('./config')

function errMessage(res) {
  return (res && res.data && res.data.message) || '请求失败'
}

function request({ url, method = 'GET', data, skipAuth = false, quiet = false }, retried = false) {
  const token = skipAuth ? '' : wx.getStorageSync('token')
  return new Promise((resolve, reject) => {
    wx.request({
      url: baseUrl + url,
      method,
      data,
      timeout: 20000,
      header: {
        'content-type': 'application/json',
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
      },
      success(res) {
        if (res.statusCode === 401 && !skipAuth && !retried) {
          wx.removeStorageSync('token')
          const app = getApp()
          if (app && app.silentLogin) {
            app.silentLogin(true)
              .then(() => request({ url, method, data, skipAuth, quiet }, true))
              .then(resolve)
              .catch(reject)
            return
          }
          reject(new Error('请先登录'))
          return
        }
        if (res.statusCode >= 400) {
          const msg = errMessage(res)
          if (!quiet) wx.showToast({ title: msg, icon: 'none' })
          reject(new Error(msg))
          return
        }
        resolve(res.data && res.data.data)
      },
      fail() {
        const msg = '网络异常，请确认后端已启动'
        if (!quiet) wx.showToast({ title: msg, icon: 'none' })
        reject(new Error(msg))
      },
    })
  })
}

module.exports = { request }
