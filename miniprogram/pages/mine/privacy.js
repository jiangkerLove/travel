const { api } = require('../../utils/api')

Page({
  data: { on: false },
  onShow() {
    const user = getApp().globalData.user || wx.getStorageSync('user') || {}
    this.setData({ on: !!user.default_bill_visible })
  },
  async onChange(e) {
    const on = e.detail.value
    this.setData({ on })
    const user = await api.updateUser({ default_bill_visible: on })
    getApp().setUser(user)
  },
})
