const { api } = require('../../utils/api')

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
  onChooseAvatar(e) {
    const avatar = e.detail.avatarUrl
    api.updateUser({ avatar }).then((user) => {
      getApp().setUser(user)
      this.applyUser(user)
    })
  },
  onNickBlur(e) {
    const nickname = (e.detail.value || '').trim()
    if (!nickname || nickname === this.data.user.nickname) return
    api.updateUser({ nickname }).then((user) => {
      getApp().setUser(user)
      this.applyUser(user)
    })
  },
  goHistory() { wx.navigateTo({ url: '/pages/mine/history' }) },
  goPrivacy() { wx.navigateTo({ url: '/pages/mine/privacy' }) },
  goAbout() { wx.navigateTo({ url: '/pages/mine/about' }) },
})
