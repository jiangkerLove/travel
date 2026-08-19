const { api } = require('../../utils/api')

Page({
  data: {
    nickname: '',
    avatar: '',
  },
  onShow() {},
  onNick(e) {
    this.setData({ nickname: e.detail.value })
  },
  onChooseAvatar(e) {
    this.setData({ avatar: e.detail.avatarUrl })
  },
  async onWxLogin() {
    wx.showLoading({ title: '进入中' })
    try {
      await getApp().silentLogin()
      wx.switchTab({ url: '/pages/index/index' })
    } finally {
      wx.hideLoading()
    }
  },
  async onDemo(e) {
    const { id, name } = e.currentTarget.dataset
    wx.showLoading({ title: '切换中' })
    try {
      const data = await api.login({ open_id: id, nickname: name })
      getApp().setUser(data.user, data.token)
      wx.switchTab({ url: '/pages/index/index' })
    } finally {
      wx.hideLoading()
    }
  },
  async onSeed() {
    wx.showLoading({ title: '导入中' })
    try {
      const data = await api.seed()
      wx.showToast({ title: `邀请码 ${data.invite_code}`, icon: 'none' })
    } finally {
      wx.hideLoading()
    }
  },
})
