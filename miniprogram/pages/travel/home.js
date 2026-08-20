const { api } = require('../../utils/api')
const {
  POINT_TYPES,
  pointMeta,
  trafficLabel,
  costLabel,
  TRAFFIC_TYPES,
  linesToPolyline,
  toMarkers,
  withDayStart,
  fitPointsForMap,
  openMap,
  formatLegHint,
  legDurationSeconds,
} = require('../../utils/constants')
const { fillLineRoutes } = require('../../utils/direction')

function decoratePlans(plans, startHint, showRoute) {
  const list = (plans || []).map((p) => ({
    ...p,
    color: pointMeta(p.point_type).color,
    typeLabel: pointMeta(p.point_type).label,
    trafficLabel: trafficLabel(p.traffic_type),
    legHint: showRoute ? formatLegHint(p.next_distance_m, legDurationSeconds(p)) : '',
  }))
  if (!showRoute) {
    return list.map((p) => ({ ...p, arriveText: '' }))
  }
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
    mapScope: 'day', // day | all
    days: [],
    currentDay: { plans: [], day_num: 1, date: '' },
    pointTypes: POINT_TYPES,
    trafficTypes: TRAFFIC_TYPES,
    bills: [],
    stat: {},
    members: [],
    mapLat: 30.67,
    mapLng: 104.06,
    markers: [],
    polyline: [],
    dragIndex: -1,
    canEdit: false,
    canBill: false,
    routesReady: false,
    generating: false,
  },
  onLoad(q) {
    const mode = q.mode === 'edit' ? 'edit' : 'browse'
    const id = Number(q.id) || 0
    const name = q.name ? decodeURIComponent(q.name) : ''
    const dest = q.dest ? decodeURIComponent(q.dest) : ''
    this._mapInited = false
    this._mapSeq = 0
    // 进页立刻定标题，避免先闪全局「结伴出行」
    wx.setNavigationBarTitle({
      title: mode === 'edit' ? '排行程' : (name || '旅途'),
    })
    this.setData({
      id,
      mode,
      tab: 'plan',
      routesReady: mode !== 'edit',
      mapScope: 'day',
      // 先用列表带来的名字占位，防止整页空白再闪
      trip: id
        ? {
            id,
            travel_name: name || '旅途',
            destination: dest || '',
            start_date: '',
            end_date: '',
          }
        : {},
    })
  },
  onShow() {
    if (!this.data.id) return
    if (this.data.mode === 'edit' && this._dirtyAfterEdit) {
      this._dirtyAfterEdit = false
      this.setData({ routesReady: false })
      this._mapCache = null
      this._plansLoaded = false
      this._mapDrawKey = ''
    }
    // 已加载过就不要整页重刷，否则地图/列表会一直闪
    if (this._plansLoaded && this.data.days && this.data.days.length) {
      return
    }
    this.refresh()
  },
  setTab(e) {
    const tab = e.currentTarget.dataset.v
    this.setData({ tab })
    if (tab === 'plan') {
      if (!this._plansLoaded) this.loadPlans()
      else this.renderMap({ fit: false })
    } else if (tab === 'bill') this.loadBills()
    else if (tab === 'member') this.loadMembers()
  },
  async refresh() {
    const id = this.data.id
    const trip = await api.travelDetail(id)
    const readOnly = !!trip.is_sample || Number(trip.status) === 2
    const canEdit = !readOnly && !!trip.can_edit
    const canBill = !readOnly && !!trip.can_bill
    if (readOnly && this.data.mode === 'edit') {
      wx.showToast({ title: '示例仅供查看', icon: 'none' })
    }
    if (this.data.mode === 'edit' && !canEdit) {
      wx.setNavigationBarTitle({ title: trip.travel_name || '旅途' })
      this.setData({ mode: 'browse', tab: 'plan', canEdit: false, canBill, routesReady: true })
      this.applyTrip(trip)
      await this.loadPlans()
      return
    }
    const title = this.data.mode === 'edit' ? '排行程' : (trip.travel_name || '旅途')
    wx.setNavigationBarTitle({ title })
    this.applyTrip(trip)
    this.setData({ canEdit, canBill })
    if (this.data.tab === 'plan' || this.data.mode === 'edit') await this.loadPlans()
    if (this.data.tab === 'bill') await this.loadBills()
    if (this.data.tab === 'member') await this.loadMembers()
  },
  applyTrip(trip) {
    // 按字段更新，减少整树重绘导致地图闪
    this.setData({
      'trip.id': trip.id,
      'trip.travel_name': trip.travel_name,
      'trip.destination': trip.destination,
      'trip.start_date': trip.start_date,
      'trip.end_date': trip.end_date,
      'trip.invite_code': trip.invite_code,
      'trip.role': trip.role,
      'trip.status': trip.status,
      'trip.status_text': trip.status_text,
      'trip.is_lock': trip.is_lock,
      'trip.is_sample': trip.is_sample,
      'trip.remark': trip.remark,
      'trip.can_edit': trip.can_edit,
      'trip.can_bill': trip.can_bill,
    })
  },
  async loadPlans({ withRoutes } = {}) {
    const editing = this.data.mode === 'edit'
    const showRoute = withRoutes != null
      ? withRoutes
      : (editing ? this.data.routesReady : true)
    const data = await api.planList(this.data.id, null, showRoute)
    const days = (data.days || []).map((d) => {
      const startHint = showRoute ? formatLegHint(d.start_distance_m, d.start_duration_s) : ''
      return {
        ...d,
        shortDate: (d.date || '').slice(5),
        startFrom: d.start_from || null,
        startHint,
        plans: decoratePlans(d.plans, startHint, showRoute),
      }
    })
    let dayIndex = this.data.dayIndex
    if (this.data.mapScope !== 'all' && (dayIndex < 0 || dayIndex >= days.length)) dayIndex = 0
    const scope = this.data.mapScope
    const idsKey = days.map((d) => `${d.day_num}:${(d.plans || []).map((p) => p.id).join(',')}`).join('|')
    const plansChanged = idsKey !== this._plansIdsKey
    this._plansIdsKey = idsKey
    if (plansChanged) {
      this._mapCache = null
      this._mapDrawKey = ''
    }

    this.setData({
      days,
      dayIndex: scope === 'all' ? -1 : dayIndex,
      currentDay: days[scope === 'all' ? Math.max(dayIndex, 0) : dayIndex] || { plans: [] },
      routesReady: showRoute,
    })
    this._plansLoaded = true
    await this.renderMap({ fit: plansChanged || !this._mapFitted })
  },
  switchAll() {
    if (this.data.mapScope === 'all') return
    this.setData({ mapScope: 'all', dayIndex: -1 })
    this.renderMap({ fit: true })
  },
  switchDay(e) {
    const dayIndex = Number(e.currentTarget.dataset.index)
    if (this.data.mapScope === 'day' && this.data.dayIndex === dayIndex) return
    const day = this.data.days[dayIndex] || { plans: [] }
    this.setData({ mapScope: 'day', dayIndex, currentDay: day })
    this.renderMap({ fit: true })
  },
  fitMap(points) {
    const withGeo = (points || []).filter((p) => p.latitude && p.longitude)
    if (!withGeo.length) return
    if (!this._mapInited) {
      this._mapInited = true
      this.setData({
        mapLat: Number(withGeo[0].latitude),
        mapLng: Number(withGeo[0].longitude),
      })
    }
    clearTimeout(this._fitTimer)
    this._fitTimer = setTimeout(() => {
      const ctx = wx.createMapContext('dayMap', this)
      if (!ctx || !ctx.includePoints) return
      ctx.includePoints({
        padding: [100, 80, 80, 80],
        points: fitPointsForMap(withGeo),
      })
      this._mapFitted = true
    }, 80)
  },
  /** 顶部地图：当天或全程 */
  async renderMap({ fit = false } = {}) {
    const seq = ++this._mapSeq
    const withLines = this.data.mode === 'edit' ? this.data.routesReady : true
    const scope = this.data.mapScope
    const day = this.data.currentDay
    const days = this.data.days || []

    let localPlans = []
    let markStart = false
    if (scope === 'all') {
      localPlans = days.flatMap((d) => d.plans || [])
    } else {
      localPlans = withDayStart((day && day.plans) || [], day && day.startFrom)
      markStart = !!(day && day.startFrom && day.startFrom.latitude)
      // 当天第一站也标成起点（没有跨天出发时）
      if (!markStart && localPlans.length) markStart = true
    }
    const localGeo = localPlans.filter((p) => p.latitude && p.longitude)
    const cacheKey = `spread1:${scope}:${scope === 'all' ? 'all' : (day && day.day_num)}:${withLines ? 1 : 0}:${localPlans.map((p) => p.id).join(',')}`

    if (!localGeo.length) {
      if (this.data.markers.length || this.data.polyline.length) {
        this.setData({ markers: [], polyline: [] })
      }
      return
    }

    if (this._mapCache && this._mapCache.key === cacheKey) {
      if (seq !== this._mapSeq) return
      const sameMark = this._mapCache.markers === this.data.markers
      if (!sameMark) {
        this.setData({
          markers: this._mapCache.markers,
          polyline: withLines ? this._mapCache.polyline : [],
        })
      }
      if (fit) this.fitMap(this._mapCache.points)
      return
    }

    if (!withLines) {
      const markers = toMarkers(localGeo, { markStart })
      const nextKey = `${markers.map((m) => `${m.id}:${m.latitude},${m.longitude}`).join('|')}#0`
      if (nextKey !== this._mapDrawKey) {
        this._mapDrawKey = nextKey
        this.setData({ markers, polyline: [] })
      }
      if (fit) this.fitMap(localGeo)
      return
    }

    try {
      let points = []
      let lines = []
      if (scope === 'all') {
        const data = await api.mapGlobal(this.data.id)
        if (seq !== this._mapSeq) return
        points = decoratePlans(data.points || [], '', true)
        const filled = fillLineRoutes(data.lines || [], points)
        lines = filled.lines
        markStart = false
      } else {
        const data = await api.mapDay(this.data.id, day.day_num)
        if (seq !== this._mapSeq) return
        points = decoratePlans(data.points || [], '', true)
        // 接口已把昨天停留插到最前；没有则用列表里的 startFrom 兜底
        if (
          day.startFrom
          && day.startFrom.latitude
          && points[0]
          && String(points[0].place_name || '').trim() !== String(day.startFrom.place_name || '').trim()
        ) {
          points = withDayStart(points, day.startFrom)
        }
        if (points.length) {
          points[0] = { ...points[0], isStart: true }
          markStart = true
        }
        const filled = fillLineRoutes(data.lines || [], points)
        lines = filled.lines
      }
      const markers = toMarkers(points, { markStart })
      const polyline = linesToPolyline(lines, points)
      this._mapCache = { key: cacheKey, markers, polyline, points }
      if (seq !== this._mapSeq) return
      // 内容没变就别再 setData，否则原生 map 会反复重绘闪烁
      const nextKey = `${markers.map((m) => `${m.id}:${m.latitude},${m.longitude}`).join('|')}#${polyline.length}`
      if (nextKey !== this._mapDrawKey) {
        this._mapDrawKey = nextKey
        this.setData({ markers, polyline })
      }
      if (fit) this.fitMap(points)
    } catch (e) {
      if (seq !== this._mapSeq) return
      this.setData({
        markers: toMarkers(localGeo, { markStart }),
        polyline: [],
      })
      if (fit) this.fitMap(localGeo)
    }
  },
  openFormAdd(via) {
    const days = this.data.days || []
    let dayNum = (this.data.currentDay && this.data.currentDay.day_num) || 1
    if (this.data.mapScope === 'all') {
      dayNum = (days[0] && days[0].day_num) || 1
    }
    const extra = via ? '&via=1' : ''
    this._dirtyAfterEdit = true
    wx.navigateTo({
      url: `/pages/plan/edit?travel_id=${this.data.id}&day_num=${dayNum}${extra}`,
    })
  },
  quickAddEnd() {
    if (this.data.mode !== 'edit' || !this.data.canEdit) {
      wx.showToast({ title: '没有改行程权限', icon: 'none' })
      return
    }
    if (this.data.mapScope === 'all') {
      const labels = (this.data.days || []).map((d) => `加到 D${d.day_num}`)
      if (!labels.length) return
      wx.showActionSheet({
        itemList: labels.length <= 6 ? labels : labels.slice(0, 6),
        success: (r) => {
          const day = this.data.days[r.tapIndex]
          if (!day) return
          this.setData({ mapScope: 'day', dayIndex: r.tapIndex, currentDay: day })
          this.openFormAdd(false)
        },
      })
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
      this._dirtyAfterEdit = true
      wx.navigateTo({
        url: `/pages/plan/edit?travel_id=${this.data.id}&id=${id}`,
      })
      return
    }
    const all = (this.data.days || []).flatMap((d) => d.plans || [])
    const p = all.find((i) => i.id === id) || (this.data.currentDay.plans || []).find((i) => i.id === id)
    if (p) openMap(p)
  },
  onMorePlan(e) {
    if (this.data.mode !== 'edit' || !this.data.canEdit) return
    if (this.data.mapScope === 'all') {
      wx.showToast({ title: '请先切到具体某一天', icon: 'none' })
      return
    }
    const id = Number(e.currentTarget.dataset.id)
    const plan = (this.data.currentDay.plans || []).find((p) => p.id === id)
    if (!plan) return
    const days = this.data.days || []
    const otherDays = days.filter((d) => d.day_num !== plan.day_num)
    const items = ['编辑地点']
    if (otherDays.length && otherDays.length <= 4) {
      otherDays.forEach((d) => items.push(`移到 D${d.day_num}`))
    } else if (otherDays.length > 4) {
      items.push('移到其他天…')
    }
    wx.showActionSheet({
      itemList: items,
      success: async (r) => {
        if (r.tapIndex === 0) {
          this._dirtyAfterEdit = true
          wx.navigateTo({ url: `/pages/plan/edit?travel_id=${this.data.id}&id=${id}` })
          return
        }
        if (items[r.tapIndex] === '移到其他天…') {
          this.pickDayToMove(plan)
          return
        }
        const label = items[r.tapIndex] || ''
        const m = label.match(/D(\d+)/)
        if (!m) return
        await this.movePlanToDay(plan, Number(m[1]))
      },
    })
  },
  pickDayToMove(plan) {
    const days = this.data.days || []
    const labels = days
      .filter((d) => d.day_num !== plan.day_num)
      .map((d) => `D${d.day_num} · ${d.shortDate || ''}`)
    if (!labels.length) {
      wx.showToast({ title: '没有其他天数', icon: 'none' })
      return
    }
    wx.showActionSheet({
      itemList: labels,
      success: async (r) => {
        const targets = days.filter((d) => d.day_num !== plan.day_num)
        const day = targets[r.tapIndex]
        if (day) await this.movePlanToDay(plan, day.day_num)
      },
    })
  },
  async movePlanToDay(plan, dayNum) {
    if (!plan || !dayNum || plan.day_num === dayNum) return
    wx.showLoading({ title: '移动中' })
    try {
      await api.planMove({
        travel_id: this.data.id,
        id: plan.id,
        day_num: dayNum,
      })
      this.setData({ routesReady: false, mapScope: 'day' })
      const idx = (this.data.days || []).findIndex((d) => d.day_num === dayNum)
      if (idx >= 0) this.setData({ dayIndex: idx })
      await this.loadPlans({ withRoutes: false })
      wx.showToast({ title: `已移到 D${dayNum}`, icon: 'none' })
    } finally {
      wx.hideLoading()
    }
  },
  onDragStart(e) {
    if (this.data.mode !== 'edit' || !this.data.canEdit || this.data.mapScope === 'all') return
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
    wx.showLoading({ title: '保存顺序' })
    try {
      await api.planSort({
        travel_id: this.data.id,
        day_num: day.day_num,
        ids,
      })
      this.setData({ routesReady: false })
      await this.loadPlans({ withRoutes: false })
    } finally {
      wx.hideLoading()
    }
  },
  async generateRoutes() {
    if (this.data.generating) return
    const hasAny = (this.data.days || []).some((d) => (d.plans || []).length)
    if (!hasAny) {
      wx.showToast({ title: '先排几个地点', icon: 'none' })
      return
    }
    this.setData({ generating: true })
    wx.showLoading({ title: '预览行程中', mask: true })
    try {
      this._mapCache = null
      this._mapDrawKey = ''
      await this.loadPlans({ withRoutes: true })
      wx.showToast({ title: '预览完成', icon: 'success' })
    } catch (e) {
      wx.showToast({ title: (e && e.message) || '预览失败', icon: 'none' })
    } finally {
      this.setData({ generating: false })
      wx.hideLoading()
    }
  },
  async finishEdit() {
    if (this.data.generating) return
    const hasAny = (this.data.days || []).some((d) => (d.plans || []).length)
    this.setData({ generating: true })
    wx.showLoading({ title: hasAny ? '保存并预览' : '保存中', mask: true })
    try {
      this._mapCache = null
      this._mapDrawKey = ''
      if (hasAny && !this.data.routesReady) {
        await this.loadPlans({ withRoutes: true })
      }
      this.setData({ mode: 'browse', tab: 'plan', routesReady: true })
      wx.setNavigationBarTitle({ title: this.data.trip.travel_name || '旅途' })
      await this.renderMap({ fit: false })
    } catch (e) {
      wx.showToast({ title: (e && e.message) || '保存失败', icon: 'none' })
    } finally {
      this.setData({ generating: false })
      wx.hideLoading()
    }
  },
  onMarker(e) {
    const all = (this.data.days || []).flatMap((d) => d.plans || [])
    const p = all.find((i) => i.id === e.detail.markerId)
    if (!p) return
    if (this.data.mode === 'edit' && this.data.canEdit) {
      this._dirtyAfterEdit = true
      wx.navigateTo({ url: `/pages/plan/edit?travel_id=${this.data.id}&id=${p.id}` })
      return
    }
    openMap(p)
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
