const { api } = require('./utils/api')

function wxLoginCode() {
  return new Promise((resolve, reject) => {
    wx.login({
      success: (res) => {
        if (res.code) resolve(res.code)
        else reject(new Error('未获取到微信登录码'))
      },
      fail: () => reject(new Error('微信登录失败')),
    })
  })
}

function localOpenId() {
  let id = wx.getStorageSync('wx_open_id')
  if (!id) {
    id = `mp_${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`
    wx.setStorageSync('wx_open_id', id)
  }
  return id
}

App({
  globalData: {
    user: null,
  },
  markTripsDirty() {
    this._tripsDirty = true
  },
  consumeTripsDirty() {
    const dirty = !!this._tripsDirty
    this._tripsDirty = false
    return dirty
  },
  onLaunch() {
    this.silentLogin().catch(() => {})
  },
  setUser(user, token) {
    this.globalData.user = user
    if (token) wx.setStorageSync('token', token)
    wx.setStorageSync('user', user)
    if (user && user.open_id) wx.setStorageSync('wx_open_id', user.open_id)
    this._userReady = !!(user && user.open_id)
  },
  silentLogin(force) {
    if (!force && this._userReady && this.globalData.user && this.globalData.user.open_id && wx.getStorageSync('token')) {
      return Promise.resolve(this.globalData.user)
    }
    if (this._loginP) return this._loginP
    this._userReady = false
    this._loginP = this._doSilentLogin()
      .then((user) => {
        this._userReady = true
        return user
      })
      .finally(() => {
        this._loginP = null
      })
    return this._loginP
  },
  async _doSilentLogin() {
    const code = await wxLoginCode()
    const data = await api.login({
      code,
      open_id: localOpenId(),
    })
    if (!data || !data.user || !data.user.open_id) {
      throw new Error('未拿到微信 OpenID，请重启后端后再试')
    }
    this.setUser(data.user, data.token)
    return data.user
  },
  ensureLogin() {
    return this.silentLogin()
      .then((user) => !!(user && user.open_id))
      .catch(() => false)
  },
  refreshUser() {
    return api.userInfo().then((user) => {
      this.setUser(user)
      return user
    })
  },
})
