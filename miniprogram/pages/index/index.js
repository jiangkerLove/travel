const { api } = require('../../utils/api')

function prettyDate(s) {
  if (!s) return ''
  const p = String(s).split('-')
  if (p.length !== 3) return s
  return `${Number(p[1])}月${Number(p[2])}日`
}

function decorate(list) {
  return (list || []).map((t) => ({
    ...t,
    rangeText: `${prettyDate(t.start_date)} – ${prettyDate(t.end_date)}`,
    countdown: t.countdown || '',
    // 以后端为准（示例/归档已强制只读）
    can_edit: !!t.can_edit,
    can_bill: !!t.can_bill,
  }))
}

Page({
  data: { list: [], loading: true },
  async onShow() {
    const ok = await getApp().ensureLogin()
    if (!ok) {
      wx.reLaunch({ url: '/pages/boot/boot' })
      return
    }

    const first = !this._inited
    const dirty = getApp().consumeTripsDirty()
    if (first || dirty) {
      await this.load({ silent: !first })
      this._inited = true
    }
  },
  onPullDownRefresh() {
    this.load({ silent: true }).finally(() => wx.stopPullDownRefresh())
  },
  async load({ silent = false } = {}) {
    if (!silent && !this.data.list.length) this.setData({ loading: true })
    try {
      const list = decorate(await api.travelList(false))
      const prev = JSON.stringify(this.data.list || [])
      const next = JSON.stringify(list)
      const patch = {}
      if (prev !== next) patch.list = list
      if (this.data.loading) patch.loading = false
      if (Object.keys(patch).length) this.setData(patch)
    } catch (e) {
      if (this.data.loading) this.setData({ loading: false })
    }
  },
  goCreate() {
    wx.navigateTo({ url: '/pages/travel/create' })
  },
  goJoin() {
    wx.navigateTo({ url: '/pages/travel/join' })
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
