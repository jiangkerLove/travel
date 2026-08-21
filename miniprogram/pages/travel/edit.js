const { api } = require('../../utils/api')

function dayCount(start, end) {
  if (!start || !end || end < start) return 0
  const a = new Date(start.replace(/-/g, '/'))
  const b = new Date(end.replace(/-/g, '/'))
  return Math.floor((b - a) / 86400000) + 1
}

function syncReady(data) {
  const { travel_name, destination, start_date, end_date } = data
  const ready = !!(
    (travel_name || '').trim() &&
    (destination || '').trim() &&
    start_date &&
    end_date &&
    end_date >= start_date
  )
  return {
    ready,
    dayCount: dayCount(start_date, end_date),
  }
}

Page({
  data: {
    id: 0,
    travel_name: '',
    destination: '',
    start_date: '',
    end_date: '',
    remark: '',
    originEnd: '',
    ready: false,
    dayCount: 0,
    busy: false,
  },
  async onLoad(q) {
    const id = Number(q.id || 0)
    if (!id) {
      wx.showToast({ title: '参数错误', icon: 'none' })
      return
    }
    this.setData({ id })
    wx.showLoading({ title: '加载中' })
    try {
      const trip = await api.travelDetail(id)
      const patch = {
        travel_name: trip.travel_name || '',
        destination: trip.destination || '',
        start_date: trip.start_date || '',
        end_date: trip.end_date || '',
        remark: trip.remark || '',
        originEnd: trip.end_date || '',
        originDays: trip.day_count || 0,
      }
      this.setData({ ...patch, ...syncReady(patch) })
    } finally {
      wx.hideLoading()
    }
  },
  onName(e) {
    const travel_name = e.detail.value
    this.setData({ travel_name, ...syncReady({ ...this.data, travel_name }) })
  },
  onDest(e) {
    const destination = e.detail.value
    this.setData({ destination, ...syncReady({ ...this.data, destination }) })
  },
  onStart(e) {
    const start_date = e.detail.value
    const patch = { start_date }
    if (this.data.end_date && this.data.end_date < start_date) patch.end_date = ''
    const next = { ...this.data, ...patch }
    this.setData({ ...patch, ...syncReady(next) })
  },
  onEnd(e) {
    const end_date = e.detail.value
    this.setData({ end_date, ...syncReady({ ...this.data, end_date }) })
  },
  onRemark(e) {
    this.setData({ remark: e.detail.value })
  },
  async submit() {
    if (!this.data.ready || this.data.busy) return
    const { id, travel_name, destination, start_date, end_date, remark, dayCount, originDays } = this.data
    const doSave = async () => {
      this.setData({ busy: true })
      wx.showLoading({ title: '保存中' })
      try {
        await api.travelUpdate({
          travel_id: id,
          travel_name: travel_name.trim(),
          destination: destination.trim(),
          start_date,
          end_date,
          remark: (remark || '').trim(),
        })
        getApp().markTripsDirty && getApp().markTripsDirty()
        wx.showToast({ title: '已保存', icon: 'success' })
        setTimeout(() => wx.navigateBack(), 400)
      } finally {
        wx.hideLoading()
        this.setData({ busy: false })
      }
    }
    if (dayCount < (originDays || 0)) {
      wx.showModal({
        title: '缩短日期',
        content: `新行程共 ${dayCount} 天，第 ${dayCount + 1} 天及之后的行程点将被删除，是否继续？`,
        success: (r) => {
          if (r.confirm) doSave()
        },
      })
      return
    }
    await doSave()
  },
})
