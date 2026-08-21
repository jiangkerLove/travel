const { api } = require('../../utils/api')

Page({
  data: {
    nickname: '',
    avatar: '',
    busy: false,
  },
  onNick(e) {
    this.setData({ nickname: e.detail.value })
  },
  onChooseAvatar(e) {
    this.setData({ avatar: e.detail.avatarUrl })
  },
  async onWxLogin() {
    if (this.data.busy) return
    this.setData({ busy: true })
    wx.showLoading({ title: '进入中' })
    try {
      await getApp().silentLogin()
      const nickname = (this.data.nickname || '').trim()
      const avatar = this.data.avatar
      if (nickname || avatar) {
        const payload = {}
        if (nickname) payload.nickname = nickname
        if (avatar) payload.avatar = avatar
        const user = await api.updateUser(payload)
        getApp().setUser(user)
      }
      wx.switchTab({ url: '/pages/index/index' })
    } finally {
      wx.hideLoading()
      this.setData({ busy: false })
    }
  },
})
