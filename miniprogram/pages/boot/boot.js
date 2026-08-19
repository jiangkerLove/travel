Page({
  data: {
    fail: false,
    message: '',
  },
  onShow() {
    this.enter()
  },
  retry() {
    this.enter()
  },
  async enter() {
    if (this._entering) return
    this._entering = true
    this.setData({ fail: false, message: '' })
    try {
      const user = await getApp().silentLogin()
      if (!user || !user.open_id) {
        throw new Error('未拿到微信 OpenID')
      }
      wx.reLaunch({ url: '/pages/index/index' })
    } catch (e) {
      const message = (e && e.message) || '微信登录失败，请确认后端已启动'
      this.setData({ fail: true, message })
    } finally {
      this._entering = false
    }
  },
})
