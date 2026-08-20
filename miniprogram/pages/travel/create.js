const { api } = require('../../utils/api')

Page({
  data: { travel_name: '', destination: '', start_date: '', end_date: '', remark: '' },
  onName(e) { this.setData({ travel_name: e.detail.value }) },
  onDest(e) { this.setData({ destination: e.detail.value }) },
  onRemark(e) { this.setData({ remark: e.detail.value }) },
  onStart(e) { this.setData({ start_date: e.detail.value }) },
  onEnd(e) { this.setData({ end_date: e.detail.value }) },
  async submit() {
    const { travel_name, destination, start_date, end_date, remark } = this.data
    if (!travel_name || !destination || !start_date || !end_date) {
      wx.showToast({ title: '请完善必填信息', icon: 'none' })
      return
    }
    wx.showLoading({ title: '创建中' })
    try {
      const t = await api.travelCreate({ travel_name, destination, start_date, end_date, remark })
      getApp().markTripsDirty()
      wx.hideLoading()
      wx.showModal({
        title: '创建成功',
        content: `邀请码 ${t.invite_code}，可分享给同行好友`,
        showCancel: false,
        success: () => {
          wx.redirectTo({
            url: `/pages/travel/home?id=${t.id}&mode=edit&name=${encodeURIComponent(t.travel_name || travel_name)}&dest=${encodeURIComponent(t.destination || destination)}`,
          })
        },
      })
    } catch (e) {
      wx.hideLoading()
    }
  },
})
