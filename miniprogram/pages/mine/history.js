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
    const id = e.currentTarget.dataset.id
    const item = (this.data.list || []).find((t) => t.id === Number(id))
    const name = encodeURIComponent((item && item.travel_name) || '')
    const dest = encodeURIComponent((item && item.destination) || '')
    wx.navigateTo({
      url: `/pages/travel/home?id=${id}&mode=browse&name=${name}&dest=${dest}`,
    })
  },
})
