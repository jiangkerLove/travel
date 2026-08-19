const { api } = require('../../utils/api')
const { POINT_TYPES, pointMeta, trafficLabel, costLabel, TRAFFIC_TYPES, linesToPolyline, toMarkers, openMap, formatLegHint, withLegHints, legDurationSeconds } = require('../../utils/constants')
const { fillLineRoutes } = require('../../utils/direction')

function decoratePlans(plans, startHint) {
  const list = (plans || []).map((p) => ({
    ...p,
    color: pointMeta(p.point_type).color,
    typeLabel: pointMeta(p.point_type).label,
    trafficLabel: trafficLabel(p.traffic_type),
    legHint: formatLegHint(p.next_distance_m, legDurationSeconds(p)),
  }))
  return list.map((p, i) => {
    const prev = list[i - 1]
    const arriveText = i === 0
      ? [p.trafficLabel, startHint].filter(Boolean).join(' · ')
      : [p.trafficLabel, prev && prev.legHint].filter(Boolean).join(' · ')
    return { ...p, arriveText }
  })
}

Page({
  data: {
    id: 0,
    mode: 'browse',
    tab: 'plan',
    trip: {},
    dayIndex: 0,
    days: [],
    currentDay: { plans: [], day_num: 1, date: '' },
    filter: 'all',
    pointTypes: POINT_TYPES,
    trafficTypes: TRAFFIC_TYPES,
    bills: [],
    stat: {},
    members: [],
    mapLat: 30.67,
    mapLng: 104.06,
    markers: [],
    polyline: [],
    includePoints: [],
    visiblePoints: [],
    allPoints: [],
    routeHint: '',
    dragIndex: -1,
    canEdit: false,
    canBill: false,
  },
  onLoad(q) {
    const mode = q.mode === 'edit' ? 'edit' : 'browse'
    this.setData({
      id: Number(q.id),
      mode,
      tab: 'plan',
    })
  },
  onShow() {
    if (!this.data.id) return
    this.refresh()
  },
  onTab(e) {
    this.setTab({ currentTarget: { dataset: { v: e.detail.value } } })
  },
  setTab(e) {
    const tab = e.currentTarget.dataset.v
    if (tab === 'map') {
      this.ensureMap().then(() => this.setData({ tab }))
      return
    }
    this.setData({ tab })
    if (tab === 'plan') this.loadPlans()
    else if (tab === 'bill') this.loadBills()
    else if (tab === 'member') this.loadMembers()
  },
  async refresh() {
    const id = this.data.id
    const trip = await api.travelDetail(id)
    const canEdit = !!trip.can_edit
    const canBill = !!trip.can_bill
    if (this.data.mode === 'edit' && !canEdit) {
      wx.showToast({ title: '没有改行程权限', icon: 'none' })
      this.setData({ mode: 'browse', tab: 'plan', trip, canEdit, canBill })
      await this.loadPlans()
      return
    }
    wx.setNavigationBarTitle({ title: trip.travel_name })
    this.setData({ trip, canEdit, canBill })
    if (this.data.tab === 'plan') await this.loadPlans()
    if (this.data.tab === 'map') await this.ensureMap(true)
    if (this.data.tab === 'bill') await this.loadBills()
    if (this.data.tab === 'member') await this.loadMembers()
  },
  async loadPlans() {
    const data = await api.planList(this.data.id)
    const days = (data.days || []).map((d) => {
      const startHint = formatLegHint(d.start_distance_m, d.start_duration_s)
      return {
        ...d,
        shortDate: (d.date || '').slice(5),
        startFrom: d.start_from || null,
        startHint,
        plans: decoratePlans(d.plans, startHint),
      }
    })
    const dayIndex = Math.min(this.data.dayIndex, Math.max(days.length - 1, 0))
    this.setData({ days, dayIndex, currentDay: days[dayIndex] || { plans: [] } })
    this._mapLoaded = false
  },
  switchDay(e) {
    const dayIndex = e.currentTarget.dataset.index
    this.setData({ dayIndex, currentDay: this.data.days[dayIndex] || { plans: [] } })
  },
  openFormAdd(via) {
    const day = this.data.currentDay
    const extra = via ? '&via=1' : ''
    wx.navigateTo({
      url: `/pages/plan/edit?travel_id=${this.data.id}&day_num=${day.day_num || 1}${extra}`,
    })
  },
  quickAddEnd() {
    if (this.data.mode !== 'edit' || !this.data.canEdit) {
      wx.showToast({ title: '没有改行程权限', icon: 'none' })
      return
    }
    const hasPlans = !!(this.data.currentDay.plans || []).length
    if (!hasPlans) {
      this.openFormAdd(false)
      return
    }
    wx.showActionSheet({
      itemList: ['添加地点', '添加途经点'],
      success: (r) => {
        if (r.tapIndex === 0) this.openFormAdd(false)
        if (r.tapIndex === 1) this.openFormAdd(true)
      },
    })
  },
  editPlan(e) {
    if (this._justDragged) return
    const id = e.currentTarget.dataset.id
    if (this.data.mode === 'edit' && this.data.canEdit) {
      wx.navigateTo({
        url: `/pages/plan/edit?travel_id=${this.data.id}&id=${id}`,
      })
      return
    }
    const p = (this.data.currentDay.plans || []).find((i) => i.id === id)
    if (p) openMap(p)
  },
  onDragStart(e) {
    if (this.data.mode !== 'edit' || !this.data.canEdit) return
    const from = Number(e.currentTarget.dataset.index)
    this._drag = {
      startY: e.touches[0].clientY,
      from,
      current: from,
      height: 64,
      orig: (this.data.currentDay.plans || []).map((p) => p.id).join(','),
    }
    this.setData({ dragIndex: from })
    wx.createSelectorQuery()
      .in(this)
      .select('.plan-row')
      .boundingClientRect((rect) => {
        if (rect && this._drag) this._drag.height = rect.height + 8
      })
      .exec()
    wx.vibrateShort({ type: 'light' })
  },
  onDragMove(e) {
    const drag = this._drag
    if (!drag || !e.touches[0]) return
    const plans = this.data.currentDay.plans || []
    if (plans.length < 2) return
    const delta = Math.round((e.touches[0].clientY - drag.startY) / drag.height)
    const to = Math.max(0, Math.min(plans.length - 1, drag.from + delta))
    if (to === drag.current) return
    const list = plans.slice()
    const [item] = list.splice(drag.current, 1)
    list.splice(to, 0, item)
    drag.current = to
    drag.from = to
    drag.startY = e.touches[0].clientY
    this._justDragged = true
    this.setData({ 'currentDay.plans': list, dragIndex: to })
  },
  async onDragEnd() {
    const drag = this._drag
    this._drag = null
    this.setData({ dragIndex: -1 })
    setTimeout(() => { this._justDragged = false }, 200)
    if (!drag) return
    const ids = (this.data.currentDay.plans || []).map((p) => p.id)
    if (ids.join(',') === drag.orig) return
    const day = this.data.currentDay
    wx.showLoading({ title: '重算路线' })
    try {
      await api.planSort({
        travel_id: this.data.id,
        day_num: day.day_num,
        ids,
      })
      await this.loadPlans()
    } finally {
      wx.hideLoading()
    }
  },
  openDayMap() {
    const day = this.data.currentDay
    wx.navigateTo({ url: `/pages/plan/day-map?travel_id=${this.data.id}&day_num=${day.day_num || 1}` })
  },
  openPlaceMap(e) {
    const id = e.currentTarget.dataset.id
    const p = (this.data.currentDay.plans || [])
      .concat(this.data.visiblePoints || [])
      .concat(this.data.allPoints || [])
      .find((i) => i.id === id)
    if (p) openMap(p)
  },
  addBillForPlan(e) {
    wx.navigateTo({
      url: `/pages/bill/edit?travel_id=${this.data.id}&day_plan_id=${e.currentTarget.dataset.id}`,
    })
  },
  async ensureMap(force) {
    if (this._mapLoaded && !force) return
    await this.loadMap()
    this._mapLoaded = true
  },
  async loadMap() {
    const data = await api.mapGlobal(this.data.id)
    const rawPoints = decoratePlans(data.points || [])
    const filled = fillLineRoutes(data.lines || [], rawPoints)
    const allPoints = withLegHints(rawPoints, filled.lines)
    this.setData({
      allPoints,
      routeHint: filled.fromNav
        ? '路书：按高德驾车/步行路线简化后的行程概览，含大概路程和时间'
        : '路书：点到点行程概览（高铁/飞机为估算）。配置高德 Key 后沿道路规划',
    })
    this._lines = filled.lines
    this.applyFilter(this.data.filter, allPoints, filled.lines)
  },
  setFilter(e) {
    const filter = e.currentTarget.dataset.v
    this.setData({ filter })
    this.applyFilter(filter, this.data.allPoints, this._lines || [])
  },
  applyFilter(filter, points, lines) {
    const visible = filter === 'all' ? points : points.filter((p) => p.point_type === filter)
    const withGeo = visible.filter((p) => p.latitude && p.longitude)
    const markers = toMarkers(withGeo)
    const idSet = new Set(visible.map((p) => p.id))
    const filteredLines = (lines || []).filter((l) => idSet.has(l.from_id) && idSet.has(l.to_id))
    const polyline = linesToPolyline(filteredLines, points)
    const includePoints = withGeo.map((p) => ({ latitude: p.latitude, longitude: p.longitude }))
    const center = withGeo[0] || { latitude: this.data.mapLat, longitude: this.data.mapLng }
    const patch = {
      visiblePoints: visible,
      markers,
      polyline,
    }
    if (!this._mapInited && withGeo.length) {
      patch.mapLat = center.latitude
      patch.mapLng = center.longitude
      this._mapInited = true
    }
    this.setData(patch)
    if (includePoints.length) {
      wx.nextTick(() => {
        const ctx = wx.createMapContext('routeMap', this)
        if (ctx && ctx.includePoints) {
          ctx.includePoints({ padding: [40, 40, 40, 40], points: includePoints })
        }
      })
    }
  },
  onMarker(e) {
    const p = this.data.visiblePoints.find((i) => i.id === e.detail.markerId)
    if (p) openMap(p)
  },
  focusPoint(e) {
    const p = this.data.visiblePoints.find((i) => i.id === e.currentTarget.dataset.id)
    if (p && p.latitude) {
      const ctx = wx.createMapContext('routeMap', this)
      if (ctx && ctx.includePoints) {
        ctx.includePoints({
          padding: [80, 80, 80, 80],
          points: [{ latitude: p.latitude, longitude: p.longitude }],
        })
      }
      openMap(p)
    }
  },
  async loadBills() {
    const [bills, stat] = await Promise.all([api.billList(this.data.id), api.statTotal(this.data.id)])
    stat.categories = (stat.categories || []).map((c) => ({ ...c, label: costLabel(c.cost_type) }))
    this.setData({ bills: bills || [], stat })
  },
  addBill() {
    if (!this.data.canBill) {
      wx.showToast({ title: '没有记账权限', icon: 'none' })
      return
    }
    wx.navigateTo({ url: `/pages/bill/edit?travel_id=${this.data.id}` })
  },
  editBill(e) {
    if (!this.data.canBill) return
    wx.navigateTo({ url: `/pages/bill/edit?travel_id=${this.data.id}&id=${e.currentTarget.dataset.id}` })
  },
  goSettle() {
    wx.navigateTo({ url: `/pages/settle/settle?travel_id=${this.data.id}` })
  },
  async loadMembers() {
    const members = await api.travelMember(this.data.id)
    this.setData({ members: members || [] })
  },
  copyCode() {
    wx.setClipboardData({ data: this.data.trip.invite_code })
  },
  async togglePerm(e) {
    if (this.data.trip.role !== 1) return
    const id = Number(e.currentTarget.dataset.id)
    const key = e.currentTarget.dataset.k
    const m = (this.data.members || []).find((i) => i.user_id === id)
    if (!m) return
    await api.travelPerm({
      travel_id: this.data.id,
      user_id: id,
      can_edit: key === 'can_edit' ? !m.can_edit : m.can_edit,
      can_bill: key === 'can_bill' ? !m.can_bill : m.can_bill,
    })
    this.loadMembers()
  },
  async removeMember(e) {
    const ok = await new Promise((resolve) => {
      wx.showModal({ title: '移除成员', content: '确定移除该成员？', success: (r) => resolve(r.confirm) })
    })
    if (!ok) return
    await api.travelRemove({ travel_id: this.data.id, user_id: e.currentTarget.dataset.id })
    this.loadMembers()
  },
  async toggleLock() {
    await api.travelLock({ travel_id: this.data.id, is_lock: !this.data.trip.is_lock })
    this.refresh()
  },
  async archive() {
    const ok = await new Promise((resolve) => {
      wx.showModal({ title: '归档', content: '归档后将移入历史旅途', success: (r) => resolve(r.confirm) })
    })
    if (!ok) return
    await api.travelArchive(this.data.id)
    getApp().markTripsDirty()
    wx.switchTab({ url: '/pages/index/index' })
  },
  async quit() {
    const ok = await new Promise((resolve) => {
      wx.showModal({ title: '退出', content: '确定退出该旅途？', success: (r) => resolve(r.confirm) })
    })
    if (!ok) return
    await api.travelQuit(this.data.id)
    getApp().markTripsDirty()
    wx.switchTab({ url: '/pages/index/index' })
  },
})
