const { api } = require('../../utils/api')

Page({
  data: { list: [] },
  onShow() { this.load() },
  onPullDownRefresh() { this.load().finally(() => wx.stopPullDownRefresh()) },
  async load() {
    const list = await api.travelList(true)
    this.setData({ list: list || [] })
  },
  openTrip(e) {
    wx.navigateTo({ url: `/pages/travel/home?id=${e.currentTarget.dataset.id}&mode=browse` })
  },
})
