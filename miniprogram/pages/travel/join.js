const { api } = require('../../utils/api')

Page({
  data: { code: '' },
  async onShow() {
    const review = await getApp().refreshFlags()
    if (!review) return
    const pages = getCurrentPages()
    if (pages.length > 1) wx.navigateBack()
    else wx.switchTab({ url: '/pages/index/index' })
  },
  onCode(e) { this.setData({ code: (e.detail.value || '').toUpperCase() }) },
  async submit() {
    if (!this.data.code) {
      wx.showToast({ title: '请输入邀请码', icon: 'none' })
      return
    }
    wx.showLoading({ title: '加入中' })
    try {
      const t = await api.travelJoin(this.data.code)
      getApp().markTripsDirty()
      wx.redirectTo({
        url: `/pages/travel/home?id=${t.id}&mode=browse&name=${encodeURIComponent(t.travel_name || '')}&dest=${encodeURIComponent(t.destination || '')}`,
      })
    } finally {
      wx.hideLoading()
    }
  },
})
