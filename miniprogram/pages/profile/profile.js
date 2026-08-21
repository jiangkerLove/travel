function decorateUser(user) {
  const u = user || {}
  const name = u.nickname || '旅行者'
  return {
    ...u,
    nickname: name,
    initial: name.slice(0, 1),
  }
}

Page({
  data: { user: {}, initial: '旅' },
  async onShow() {
    const ok = await getApp().ensureLogin()
    if (!ok) {
      wx.reLaunch({ url: '/pages/boot/boot' })
      return
    }
    this.applyUser(getApp().globalData.user || wx.getStorageSync('user') || {})
  },
  applyUser(user) {
    const u = decorateUser(user)
    this.setData({ user: u, initial: u.initial })
  },
  goEdit() { wx.navigateTo({ url: '/pages/profile/edit' }) },
  goHistory() { wx.navigateTo({ url: '/pages/mine/history' }) },
  goAbout() { wx.navigateTo({ url: '/pages/mine/about' }) },
})
