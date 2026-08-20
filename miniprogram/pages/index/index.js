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
    initial: (t.destination || t.travel_name || '途').slice(0, 1),
    can_edit: t.role === 1 || !!t.can_edit,
    can_bill: t.role === 1 || !!t.can_bill,
  }))
}

const { api } = require('../../utils/api')

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
    const { id, mode } = e.currentTarget.dataset
    const item = (this.data.list || []).find((t) => t.id === Number(id))
    if (mode === 'edit' && item && !item.can_edit) {
      wx.showToast({ title: '还没有改行程权限', icon: 'none' })
      return
    }
    const name = encodeURIComponent((item && item.travel_name) || '')
    const dest = encodeURIComponent((item && item.destination) || '')
    wx.navigateTo({
      url: `/pages/travel/home?id=${id}&mode=${mode || 'browse'}&name=${name}&dest=${dest}`,
    })
  },
})
