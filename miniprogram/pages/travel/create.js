const { api } = require('../../utils/api')
const { coverOptions, ROUTE_KEY } = require('../../utils/illust')

Page({
  data: {
    travel_name: '',
    destination: '',
    start_date: '',
    end_date: '',
    remark: '',
    cover: ROUTE_KEY,
    covers: coverOptions(),
    ready: false,
  },
  syncReady() {
    const { travel_name, destination, start_date, end_date } = this.data
    const ready = !!(travel_name.trim() && destination.trim() && start_date && end_date)
    if (ready !== this.data.ready) this.setData({ ready })
  },
  onName(e) {
    this.setData({ travel_name: e.detail.value })
    this.syncReady()
  },
  onDest(e) {
    this.setData({ destination: e.detail.value })
    this.syncReady()
  },
  onRemark(e) {
    this.setData({ remark: e.detail.value })
  },
  onCover(e) {
    const cover = (e.currentTarget.dataset && e.currentTarget.dataset.key) || ROUTE_KEY
    this.setData({ cover })
  },
  onStart(e) {
    const start_date = e.detail.value
    const patch = { start_date }
    if (this.data.end_date && this.data.end_date < start_date) patch.end_date = ''
    this.setData(patch)
    this.syncReady()
  },
  onEnd(e) {
    this.setData({ end_date: e.detail.value })
    this.syncReady()
  },
  async submit() {
    const travel_name = this.data.travel_name.trim()
    const destination = this.data.destination.trim()
    const { start_date, end_date, remark, ready } = this.data
    if (!ready) {
      wx.showToast({ title: '请先填完名称、目的地和日期', icon: 'none' })
      return
    }
    if (this._submitting) return
    this._submitting = true
    wx.showLoading({ title: '创建中', mask: true })
    try {
      const t = await api.travelCreate({
        travel_name,
        destination,
        start_date,
        end_date,
        remark: (remark || '').trim() || undefined,
        cover: this.data.cover || ROUTE_KEY,
      })
      getApp().markTripsDirty()
      wx.hideLoading()
      wx.showModal({
        title: '旅途已创建',
        content: `邀请码 ${t.invite_code}，可分享给同行好友`,
        confirmText: '去排行程',
        showCancel: false,
        success: () => {
          wx.redirectTo({
            url: `/pages/travel/home?id=${t.id}&mode=browse&openEdit=1&name=${encodeURIComponent(t.travel_name || travel_name)}&dest=${encodeURIComponent(t.destination || destination)}`,
          })
        },
      })
    } catch (e) {
      wx.hideLoading()
    } finally {
      this._submitting = false
    }
  },
})
