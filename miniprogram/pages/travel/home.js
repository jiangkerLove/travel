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
  formatLegLabel,
} = require('../../utils/constants')
const { fillLineRoutes } = require('../../utils/direction')

function todayStr() {
  const d = new Date()
  const m = `${d.getMonth() + 1}`.padStart(2, '0')
  const day = `${d.getDate()}`.padStart(2, '0')
  return `${d.getFullYear()}-${m}-${day}`
}

/** 浏览态默认：行程期内看今天，否则看全程 */
function pickBrowseScope(days, trip) {
  const list = days || []
  const today = todayStr()
  const start = (trip && trip.start_date) || (list[0] && list[0].date) || ''
  const end = (trip && trip.end_date) || (list[list.length - 1] && list[list.length - 1].date) || ''
  if (start && end && (today < start || today > end)) {
    return { mapScope: 'all', dayIndex: -1 }
  }
  const idx = list.findIndex((d) => d.date === today)
  if (idx >= 0) return { mapScope: 'day', dayIndex: idx }
  return { mapScope: 'all', dayIndex: -1 }
}

function cloneDays(days) {
  return JSON.parse(JSON.stringify(days || []))
}

function daysToDraft(days) {
  return (days || []).map((d) => ({
    day_num: d.day_num,
    points: (d.plans || []).map((p) => ({
      place_name: p.place_name,
      query: p.place_name,
      point_type: p.point_type || 'sight',
      stay_minutes: p.stay_duration || null,
      arrive: p.arrive_time || null,
      note: p.remark || null,
      longitude: p.longitude,
      latitude: p.latitude,
      found: !!(p.latitude && p.longitude),
    })),
  }))
}

function decoratePlans(plans, startHint, showRoute) {
  const list = (plans || []).map((p) => ({
    ...p,
    color: pointMeta(p.point_type).color,
    typeLabel: pointMeta(p.point_type).label,
    trafficLabel: trafficLabel(p.traffic_type),
  }))
  return list.map((p, i) => {
    const prev = list[i - 1]
    let arriveText = ''
    if (i === 0) {
      if (startHint) {
        arriveText = [p.trafficLabel, showRoute ? startHint : ''].filter(Boolean).join(' · ')
      }
    } else {
      const duration = Number(p.traffic_duration) > 0
        ? Number(p.traffic_duration) * 60
        : (showRoute ? Number(prev.next_duration_s) || 0 : 0)
      const distance = showRoute ? (prev.next_distance_m || 0) : 0
      arriveText = formatLegLabel(p.traffic_type, distance, duration)
    }
    return { ...p, arriveText }
  })
}

function buildMemberGroups(members) {
  const namedOrder = []
  const namedMap = new Map()
  const solos = []
  for (const m of members) {
    const name = (m.group_name || '').trim()
    if (name) {
      if (!namedMap.has(name)) {
        namedMap.set(name, [])
        namedOrder.push(name)
      }
      namedMap.get(name).push(m)
    } else {
      solos.push(m)
    }
  }
  const groups = namedOrder.map((name) => ({
    key: `g:${name}`,
    title: name,
    isGroup: true,
    members: namedMap.get(name),
  }))
  if (solos.length) {
    groups.push({
      key: 'solos',
      title: '各自单独结算',
      isGroup: false,
      members: solos,
    })
  }
  return groups
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
    billScope: '全程',
    billView: 'pool',
    billListTitle: '账单池明细',
    billEmptyTitle: '还没有账单',
    billEmptySub: '右下角加号记一笔餐饮或购物',
    stat: {},
    members: [],
    memberGroups: [],
    mapLat: 30.67,
    mapLng: 104.06,
    markers: [],
    polyline: [],
    mapExpanded: false,
    mapHeight: 210,
    dragIndex: -1,
    canEdit: false,
    canBill: false,
    isCreator: false,
    routesReady: false,
    generating: false,
    aiOpen: false,
    aiPrompt: '',
    aiLoading: false,
    aiApplying: false,
    aiDraft: null,
    aiMissCount: 0,
    aiDayNum: 0,
    aiMode: 'plan',
    aiPending: false,
    aiEntry: false,
    dayPickOpen: false,
    dayPickTitle: '加到哪一天',
    dayPickDays: [],
  },
  onLoad(q) {
    const mode = q.mode === 'edit' ? 'edit' : 'browse'
    const id = Number(q.id) || 0
    const name = q.name ? decodeURIComponent(q.name) : ''
    const dest = q.dest ? decodeURIComponent(q.dest) : ''
    this._mapInited = false
    this._mapSeq = 0
    this._dayScopeInited = false
    // 进页立刻定标题，避免先闪全局「旅途计划」
    wx.setNavigationBarTitle({
      title: mode === 'edit' ? '排行程' : (name || '旅途'),
    })
    this.setData({
      id,
      mode,
      tab: 'plan',
      routesReady: mode !== 'edit',
      mapScope: mode === 'edit' ? 'day' : 'all',
      dayIndex: mode === 'edit' ? 0 : -1,
      mapHeight: this.collapsedMapHeight(),
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
    // 从记账页返回：立刻刷新账单
    if (this._billsDirty) {
      this._billsDirty = false
      if (this.data.tab === 'bill') this.loadBills()
      else this._billsStale = true
    }
    // 已加载过就不要整页重刷，否则地图/列表会一直闪
    if (this._plansLoaded && this.data.days && this.data.days.length) {
      return
    }
    this.refresh()
  },
  setTab(e) {
    const tab = e.currentTarget.dataset.v
    const patch = { tab }
    if (tab !== 'plan' && this.data.mapExpanded) {
      patch.mapExpanded = false
      this.animateMapHeight(this.collapsedMapHeight(), 420)
    }
    this.setData(patch)
    if (tab === 'plan') {
      if (!this._plansLoaded) this.loadPlans()
      else this.renderMap({ fit: false })
    } else if (tab === 'bill') {
      if (this._billsStale || !this._allBills) {
        this._billsStale = false
        this.loadBills()
      } else {
        this.applyBillFilter()
      }
    } else if (tab === 'member') this.loadMembers()
  },
  enterEdit() {
    if (!this.data.canEdit) {
      wx.showToast({ title: '没有改行程权限', icon: 'none' })
      return
    }
    wx.setNavigationBarTitle({ title: '排行程' })
    this.setData({ mode: 'edit', tab: 'plan' })
    if (this.data.mapScope === 'all') {
      const dayIndex = Math.max(this.data.dayIndex, 0)
      const day = this.data.days[dayIndex] || { plans: [] }
      this.setData({ mapScope: 'day', dayIndex, currentDay: day })
      this.renderMap({ fit: true })
    }
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
      this.setData({ mode: 'browse', tab: 'plan', canEdit: false, canBill, routesReady: true, aiEntry: false, aiOpen: false })
      this.applyTrip(trip)
      await this.loadPlans()
      return
    }
    const title = this.data.mode === 'edit' ? '排行程' : (trip.travel_name || '旅途')
    wx.setNavigationBarTitle({ title })
    this.applyTrip(trip)
    const user = getApp().globalData.user || wx.getStorageSync('user') || {}
    this.setData({
      canEdit,
      canBill,
      isCreator: Number(trip.creator_id) === Number(user.id),
    })
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
      'trip.creator_id': trip.creator_id,
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
    let scope = this.data.mapScope
    if (!this._dayScopeInited) {
      this._dayScopeInited = true
      if (this.data.mode === 'browse') {
        const picked = pickBrowseScope(days, this.data.trip)
        scope = picked.mapScope
        dayIndex = picked.dayIndex
      } else if (dayIndex < 0 || dayIndex >= days.length) {
        dayIndex = 0
        scope = 'day'
      }
    } else if (scope !== 'all' && (dayIndex < 0 || dayIndex >= days.length)) {
      dayIndex = 0
    }
    const idsKey = days.map((d) => `${d.day_num}:${(d.plans || []).map((p) => p.id).join(',')}`).join('|')
    const plansChanged = idsKey !== this._plansIdsKey
    this._plansIdsKey = idsKey
    if (plansChanged) {
      this._mapCache = null
      this._mapDrawKey = ''
    }

    this.setData({
      days,
      mapScope: scope,
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
    if (this.data.tab === 'bill') this.applyBillFilter()
  },
  switchDay(e) {
    const dayIndex = Number(e.currentTarget.dataset.index)
    if (this.data.mapScope === 'day' && this.data.dayIndex === dayIndex) return
    const day = this.data.days[dayIndex] || { plans: [] }
    this.setData({ mapScope: 'day', dayIndex, currentDay: day })
    this.renderMap({ fit: true })
    if (this.data.tab === 'bill') this.applyBillFilter()
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
        padding: [52, 40, 40, 40],
        points: fitPointsForMap(withGeo),
      })
      this._mapFitted = true
    }, 80)
  },
  setMapView(markers, polyline) {
    const poly = polyline || []
    const nextKey = `${(markers || []).map((m) => `${m.id}:${m.latitude},${m.longitude}:${(m.callout && m.callout.content) || (m.label && m.label.content) || ''}`).join('|')}#${poly.length}`
    if (nextKey !== this._mapDrawKey) {
      this._mapDrawKey = nextKey
      this.setData({ markers: markers || [], polyline: poly })
    }
  },
  collapsedMapHeight() {
    return Math.round(420 * wx.getSystemInfoSync().windowWidth / 750)
  },
  expandedMapHeight() {
    const sys = wx.getSystemInfoSync()
    const rpx = sys.windowWidth / 750
    const hero = Math.round(56 * rpx)
    const days = Math.round(92 * rpx)
    const pad = Math.round(24 * rpx)
    const bar = this.data.mode === 'edit'
      ? Math.round(108 * rpx) + ((sys.safeAreaInsets && sys.safeAreaInsets.bottom) || 0)
      : Math.round(12 * rpx)
    return Math.max(280, Math.round(sys.windowHeight - hero - days - pad - bar))
  },
  animateMapHeight(to, duration) {
    const from = Number(this.data.mapHeight) || this.collapsedMapHeight()
    if (this._hTimer) {
      clearTimeout(this._hTimer)
      this._hTimer = null
    }
    if (from === to) {
      this.setData({ mapHeight: to })
      return
    }
    const t0 = Date.now()
    const tick = () => {
      const t = Math.min(1, (Date.now() - t0) / duration)
      const eased = 1 - (1 - t) ** 3
      this.setData({ mapHeight: Math.round(from + (to - from) * eased) })
      if (t < 1) this._hTimer = setTimeout(tick, 32)
      else this._hTimer = null
    }
    tick()
  },
  applyMapMarkers(showLegs) {
    const cache = this._mapCache
    if (!cache || !cache.points) return
    const markers = toMarkers(cache.points, {
      markStart: cache.markStart,
      lines: cache.lines,
      showLegs,
    })
    cache.markers = markers
    this._mapDrawKey = ''
    this.setMapView(markers, cache.polyline)
  },
  toggleMapFull() {
    const mapExpanded = !this.data.mapExpanded
    const mapHeight = mapExpanded ? this.expandedMapHeight() : this.collapsedMapHeight()
    this.setData({ mapExpanded })
    this.applyMapMarkers(mapExpanded)
    this.animateMapHeight(mapHeight, 420)
    if (this._fitDelay) clearTimeout(this._fitDelay)
    this._fitDelay = setTimeout(() => {
      const pts = (this._mapCache && this._mapCache.points) || []
      if (pts.length) this.fitMap(pts)
    }, 430)
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
    const cacheKey = `spread3:${scope}:${scope === 'all' ? 'all' : (day && day.day_num)}:${withLines ? 1 : 0}:${localPlans.map((p) => p.id).join(',')}`

    if (!localGeo.length) {
      if (this.data.markers.length || this.data.polyline.length) {
        this.setData({ markers: [], polyline: [] })
      }
      return
    }

    if (this._mapCache && this._mapCache.key === cacheKey) {
      if (seq !== this._mapSeq) return
      this.applyMapMarkers(this.data.mapExpanded)
      if (fit) this.fitMap(this._mapCache.points)
      return
    }

    if (!withLines) {
      const markers = toMarkers(localGeo, { markStart, showLegs: this.data.mapExpanded })
      this._mapCache = { key: cacheKey, markers, polyline: [], points: localGeo, lines: [], markStart }
      this.setMapView(markers, [])
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
      const markers = toMarkers(points, { markStart, lines, showLegs: this.data.mapExpanded })
      const polyline = linesToPolyline(lines, points)
      this._mapCache = { key: cacheKey, markers, polyline, points, lines, markStart }
      if (seq !== this._mapSeq) return
      this.setMapView(markers, polyline)
      if (fit) this.fitMap(points)
    } catch (e) {
      if (seq !== this._mapSeq) return
      const markers = toMarkers(localGeo, { markStart, showLegs: this.data.mapExpanded })
      this._mapCache = { key: cacheKey, markers, polyline: [], points: localGeo, lines: [], markStart }
      this.setMapView(markers, [])
      if (fit) this.fitMap(localGeo)
    }
  },
  openFormAdd() {
    const days = this.data.days || []
    let dayNum = (this.data.currentDay && this.data.currentDay.day_num) || 1
    if (this.data.mapScope === 'all') {
      dayNum = (days[0] && days[0].day_num) || 1
    }
    this.openPlanEdit({ day_num: dayNum })
  },
  openPlanEdit({ day_num, id, plan } = {}) {
    if (!this.data.canEdit) {
      wx.showToast({ title: '没有改行程权限', icon: 'none' })
      return
    }
    this._dirtyAfterEdit = true
    const tid = this.data.id
    let url = `/pages/plan/edit?travel_id=${tid}`
    if (id) url += `&id=${id}`
    if (day_num) url += `&day_num=${day_num}`
    const allPlans = (this.data.days || []).flatMap((d) => d.plans || [])
    const planData = plan || (id ? allPlans.find((p) => p.id === Number(id)) : null)
    wx.navigateTo({
      url,
      success: (res) => {
        res.eventChannel.emit('init', {
          trip: this.data.trip,
          days: this.data.days || [],
          plan: planData || null,
        })
      },
    })
  },
  quickAddEnd() {
    if (this.data.mode !== 'edit' || !this.data.canEdit) {
      wx.showToast({ title: '没有改行程权限', icon: 'none' })
      return
    }
    if (this.data.mapScope === 'all') {
      this.openDayPick('add')
      return
    }
    this.openFormAdd()
  },
  openDayPick(purpose, plan) {
    const days = this.data.days || []
    if (!days.length) {
      wx.showToast({ title: '还没有行程日期', icon: 'none' })
      return
    }
    this._dayPickPurpose = purpose
    this._movePlan = plan || null
    this._dayPickIgnoreUntil = Date.now() + 400
    const cur = plan && plan.day_num
    const dayPickDays = days.map((d, index) => {
      const count = (d.plans || []).length
      const here = purpose === 'move' && d.day_num === cur
      return {
        day_num: d.day_num,
        shortDate: d.shortDate || (d.date || '').slice(5),
        date: d.date || '',
        index,
        disabled: here,
        hint: here ? '当前所在天' : (count ? `${count} 个地点` : '还空着'),
      }
    })
    this.setData({
      dayPickOpen: true,
      dayPickTitle: purpose === 'move' ? '移到哪一天' : '加到哪一天',
      dayPickDays,
    })
  },
  closeDayPick() {
    if (this._dayPickIgnoreUntil && Date.now() < this._dayPickIgnoreUntil) return
    this.setData({ dayPickOpen: false })
  },
  onPickDay(e) {
    const dayNum = Number(e.currentTarget.dataset.day)
    const row = (this.data.dayPickDays || []).find((d) => d.day_num === dayNum)
    if (!row || row.disabled) return
    this.setData({ dayPickOpen: false })
    if (this._dayPickPurpose === 'add') {
      const day = (this.data.days || [])[row.index]
        || (this.data.days || []).find((d) => d.day_num === dayNum)
      if (!day) return
      this.setData({ mapScope: 'day', dayIndex: row.index, currentDay: day })
      this.openFormAdd()
      return
    }
    if (this._dayPickPurpose === 'move' && this._movePlan) {
      this.movePlanToDay(this._movePlan, dayNum)
    }
  },
  editPlan(e) {
    if (this._justDragged) return
    const id = e.currentTarget.dataset.id
    if (this.data.mode === 'edit' && this.data.canEdit) {
      this.openPlanEdit({ id })
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
          this.openPlanEdit({ id, plan })
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
    this.openDayPick('move', plan)
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
    this._aiBackup = null
    this.setData({ aiPending: false, aiEntry: false, aiOpen: false })
    const hasAny = (this.data.days || []).some((d) => (d.plans || []).length)
    this.setData({ generating: true })
    wx.showLoading({ title: hasAny ? '保存并预览' : '保存中', mask: true })
    try {
      this._mapCache = null
      this._mapDrawKey = ''
      if (hasAny && !this.data.routesReady) {
        await this.loadPlans({ withRoutes: true })
      }
      this.setData({ mode: 'browse', tab: 'plan', routesReady: true, aiEntry: false, aiOpen: false })
      wx.setNavigationBarTitle({ title: this.data.trip.travel_name || '旅途' })
      await this.renderMap({ fit: false })
    } catch (e) {
      wx.showToast({ title: (e && e.message) || '保存失败', icon: 'none' })
    } finally {
      this.setData({ generating: false })
      wx.hideLoading()
    }
  },
  openAi({ all } = {}) {
    if (!this.data.canEdit) {
      wx.showToast({ title: '没有改行程权限', icon: 'none' })
      return
    }
    this._aiIgnoreCloseUntil = Date.now() + 500
    let aiDayNum = 0
    if (this.data.aiPending) {
      aiDayNum = this.data.aiDayNum || 0
    } else if (!all) {
      aiDayNum = 0
    }
    wx.setNavigationBarTitle({ title: 'AI 排行程' })
    this.setData({ aiOpen: true, aiDayNum, aiEntry: true, aiMode: this.data.aiMode || 'plan' })
  },
  closeAi() {
    if (this.data.aiLoading || this.data.aiApplying) return
    if (this._aiIgnoreCloseUntil && Date.now() < this._aiIgnoreCloseUntil) return
    this.setData({ aiOpen: false })
  },
  onAiMask() {},
  onAiSheetTap() {},
  onAiPrompt(e) {
    this.setData({ aiPrompt: e.detail.value })
  },
  setAiDay(e) {
    const raw = e.currentTarget.dataset.scope
    const aiDayNum = raw === 'all' || raw == null || raw === '' ? 0 : Number(raw)
    if (Number.isNaN(aiDayNum) || aiDayNum === this.data.aiDayNum) return
    this._aiIgnoreCloseUntil = Date.now() + 400
    this.setData({ aiOpen: true, aiDayNum })
  },
  setAiMode(e) {
    const aiMode = e.currentTarget.dataset.mode === 'recommend' ? 'recommend' : 'plan'
    if (aiMode === this.data.aiMode) return
    this._aiIgnoreCloseUntil = Date.now() + 400
    this.setData({ aiOpen: true, aiMode })
  },
  async cancelAi() {
    if (this.data.aiLoading) return
    const backup = this._aiBackup
    if (!backup) {
      this.setData({ aiOpen: false, aiPending: false })
      return
    }
    wx.showLoading({ title: '还原中', mask: true })
    try {
      await api.planAiApply({
        travel_id: this.data.id,
        day_num: null,
        days: daysToDraft(backup),
      })
      this._aiBackup = null
      this._mapCache = null
      this._mapDrawKey = ''
      this.setData({ aiOpen: false, aiPending: false, aiEntry: false, routesReady: false })
      await this.loadPlans({ withRoutes: false })
      wx.showToast({ title: '已还原', icon: 'none' })
    } catch (e) {
      wx.showToast({ title: (e && e.message) || '还原失败', icon: 'none' })
    } finally {
      wx.hideLoading()
    }
  },
  async runAiDraft() {
    if (this.data.aiLoading) return
    const recommend = this.data.aiMode === 'recommend'
    const prompt = (this.data.aiPrompt || '').trim()
    if (!recommend && prompt.length < 2) {
      wx.showToast({ title: this.data.aiDayNum ? '写一下这天要加什么、去哪' : '写一下要去哪，或粘贴链接', icon: 'none' })
      return
    }
    const dayNum = this.data.aiDayNum || 0
    const hasOld = dayNum
      ? !!(((this.data.days || []).find((d) => d.day_num === dayNum) || {}).plans || []).length
      : (this.data.days || []).some((d) => (d.plans || []).length)
    if (recommend && !hasOld) {
      wx.showToast({ title: '先排几个地点，再沿途推荐', icon: 'none' })
      return
    }
    if (hasOld) {
      const ok = await new Promise((resolve) => {
        wx.showModal({
          title: recommend
            ? (dayNum ? `给 D${dayNum} 加推荐？` : '沿现有路线推荐？')
            : (dayNum ? `改写 D${dayNum}？` : '按 AI 重排全程？'),
          content: recommend
            ? '会保留现有地点，在顺路位置插入推荐景点。不满意可还原。'
            : (dayNum
              ? '只改这一天，写进行程后可预览路线。不满意再还原。'
              : '会按 AI 结果写入行程，和其他地点一样。不满意可还原。'),
          confirmText: recommend ? '推荐' : '生成',
          success: (r) => resolve(r.confirm),
        })
      })
      if (!ok) return
    }
    this.setData({ aiLoading: true })
    wx.showLoading({ title: recommend ? '正在推荐景点' : (dayNum ? `正在排 D${dayNum}` : '正在排行程'), mask: true })
    try {
      if (!this._aiBackup) this._aiBackup = cloneDays(this.data.days)
      const draft = await api.planAiDraft({
        travel_id: this.data.id,
        prompt,
        day_num: dayNum || null,
        mode: recommend ? 'recommend' : 'plan',
      })
      await api.planAiApply({
        travel_id: this.data.id,
        day_num: dayNum || null,
        days: (draft && draft.days) || [],
      })
      this._mapCache = null
      this._mapDrawKey = ''
      const dayIndex = dayNum
        ? Math.max(0, (this.data.days || []).findIndex((d) => d.day_num === dayNum))
        : this.data.dayIndex
      this.setData({
        aiOpen: false,
        aiPending: true,
        aiEntry: true,
        routesReady: false,
        mapScope: dayNum ? 'day' : 'all',
        dayIndex: dayNum ? dayIndex : -1,
      })
      wx.hideLoading()
      await this.loadPlans({ withRoutes: false })
      await this.generateRoutes()
    } catch (e) {
      wx.hideLoading()
      wx.showToast({ title: (e && e.message) || '生成失败', icon: 'none' })
    } finally {
      this.setData({ aiLoading: false })
    }
  },
  onMarker(e) {
    const markerId = e.detail.markerId
    if (Number(markerId) >= 900000000) return
    const all = (this.data.days || []).flatMap((d) => d.plans || [])
    const p = all.find((i) => i.id === markerId)
    if (!p) return
    if (this.data.mode === 'edit' && this.data.canEdit) {
      this.openPlanEdit({ id: p.id, plan: p })
      return
    }
    openMap(p)
  },
  async loadBills() {
    const [bills, stat, members] = await Promise.all([
      api.billList(this.data.id),
      api.statTotal(this.data.id),
      this.data.members && this.data.members.length
        ? Promise.resolve(this.data.members)
        : api.travelMember(this.data.id),
    ])
    const user = getApp().globalData.user || wx.getStorageSync('user') || {}
    if (members && (!this.data.members || !this.data.members.length)) {
      const mapped = (members || []).map((m) => ({
        ...m,
        nickLetter: (m.nickname || '?').slice(0, 1),
      }))
      this.setData({ members: mapped, memberGroups: buildMemberGroups(mapped) })
    }
    this._allBills = (bills || []).map((b) => {
      const shareCount = (b.shares || []).length
      let share_hint = ''
      if (shareCount > 1) share_hint = `${shareCount}人分摊`
      else if (shareCount === 1) share_hint = '个人'
      else if (b.bill_type === 2) share_hint = '个人'
      return {
        ...b,
        cost_type_label: costLabel(b.cost_type),
        consume_date: (b.consume_time || '').slice(0, 10),
        share_hint,
      }
    })
    this._tripStat = {
      ...stat,
      member_count: stat.member_count || 1,
      categories: (stat.categories || []).map((c) => ({ ...c, label: costLabel(c.cost_type) })),
    }
    this._billUserId = Number(user.id) || 0
    this.applyBillFilter()
  },
  applyBillFilter() {
    const all = this._allBills || []
    const tripStat = this._tripStat || {}
    const userId = Number(this._billUserId) || 0
    const billView = this.data.billView || 'pool'
    let scoped = all
    let billScope = '全程'
    let date = ''

    if (this.data.mapScope === 'day') {
      const day = (this.data.days || [])[this.data.dayIndex] || this.data.currentDay || {}
      date = day.date || ''
      if (date) {
        scoped = all.filter((b) => b.consume_date === date)
        billScope = `D${day.day_num || ''}`
      }
    }

    const round2 = (n) => Math.round(n * 100) / 100
    const myCostOf = (b) => {
      if (b.shares && b.shares.length) {
        const share = b.shares.find((s) => Number(s.user_id) === userId)
        return share ? Number(share.share_amount) || 0 : 0
      }
      if (b.bill_type === 2 && Number(b.pay_user_id) === userId) return Number(b.amount) || 0
      return 0
    }
    const iAmIn = (b) => {
      if (b.shares && b.shares.length) {
        return b.shares.some((s) => Number(s.user_id) === userId)
      }
      return b.bill_type === 2 && Number(b.pay_user_id) === userId
    }

    let stat = tripStat
    if (date) {
      let public_total = 0
      let private_total = 0
      const catMap = {}
      for (const b of scoped) {
        const amount = Number(b.amount) || 0
        if (b.visible_all) {
          public_total += amount
          catMap[b.cost_type] = (catMap[b.cost_type] || 0) + amount
        }
        private_total += myCostOf(b)
      }
      const mc = tripStat.member_count || 1
      stat = {
        public_total: round2(public_total),
        private_total: round2(private_total),
        avg_public: round2(public_total / mc),
        member_count: mc,
        categories: Object.keys(catMap).map((cost_type) => ({
          cost_type,
          amount: round2(catMap[cost_type]),
          label: costLabel(cost_type),
        })),
      }
    }

    let bills = []
    if (billView === 'mine') {
      bills = scoped.filter(iAmIn).map((b) => {
        const mine = round2(myCostOf(b))
        const showWhole = (b.shares || []).length > 1 && mine !== Number(b.amount)
        return {
          ...b,
          display_amount: mine,
          display_sub: showWhole ? `整笔 ¥${b.amount}` : '',
        }
      })
    } else {
      bills = scoped
        .filter((b) => !!b.visible_all)
        .map((b) => ({
          ...b,
          display_amount: b.amount,
          display_sub: '',
        }))
    }

    const dayBit = billScope && billScope !== '全程' ? ` · ${billScope}` : ''
    const billListTitle = billView === 'mine' ? `我的花销${dayBit}` : `账单池${dayBit}`
    let billEmptyTitle = '还没有账单'
    let billEmptySub = '右下角加号记一笔餐饮或购物'
    if (billView === 'mine') {
      billEmptyTitle = date ? '这天没有你的花销' : '还没有你的花销'
      billEmptySub = '记一笔并勾选自己分摊即可'
    } else if (date) {
      billEmptyTitle = '这天还没有公开账单'
    }

    this.setData({ bills, stat, billScope, billListTitle, billEmptyTitle, billEmptySub })
  },
  setBillView(e) {
    const billView = e.currentTarget.dataset.v
    if (!billView || billView === this.data.billView) return
    this.setData({ billView })
    this.applyBillFilter()
  },
  addBill() {
    if (!this.data.canBill) {
      wx.showToast({ title: '没有记账权限', icon: 'none' })
      return
    }
    let url = `/pages/bill/edit?travel_id=${this.data.id}`
    if (this.data.mapScope === 'day') {
      const day = (this.data.days || [])[this.data.dayIndex]
      if (day && day.date) url += `&consume_date=${day.date}`
    }
    this._billsDirty = true
    wx.navigateTo({
      url,
      success: (res) => {
        res.eventChannel.emit('init', {
          trip: this.data.trip,
          days: this.data.days || [],
          members: this.data.members || [],
          bills: this._allBills || [],
        })
      },
    })
  },
  editBill(e) {
    if (!this.data.canBill) return
    this._billsDirty = true
    const id = e.currentTarget.dataset.id
    const bill = (this._allBills || []).find((b) => b.id === Number(id))
    wx.navigateTo({
      url: `/pages/bill/edit?travel_id=${this.data.id}&id=${id}`,
      success: (res) => {
        res.eventChannel.emit('init', {
          trip: this.data.trip,
          days: this.data.days || [],
          members: this.data.members || [],
          bills: this._allBills || [],
          bill: bill || null,
        })
      },
    })
  },
  goSettle() {
    wx.navigateTo({ url: `/pages/settle/settle?travel_id=${this.data.id}` })
  },
  async loadMembers() {
    const members = (await api.travelMember(this.data.id) || []).map((m) => ({
      ...m,
      nickLetter: (m.nickname || '?').slice(0, 1),
    }))
    this.setData({
      members,
      memberGroups: buildMemberGroups(members),
    })
  },
  onMemberTap(e) {
    const id = Number(e.currentTarget.dataset.id)
    const m = (this.data.members || []).find((i) => i.user_id === id)
    if (!m) return
    const trip = this.data.trip || {}
    const isLeader = !trip.is_sample && trip.status !== 2 && trip.role === 1
    if (!isLeader) {
      wx.showToast({
        title: m.group_name ? `${m.nickname} · ${m.group_name}` : `${m.nickname} · 单独结算`,
        icon: 'none',
      })
      return
    }

    const actions = []
    const handlers = []
    actions.push(m.group_name ? '改团体' : '加入团体')
    handlers.push(() => this.editGroup({ currentTarget: { dataset: { id } } }))
    if (m.role !== 1 && !m.is_guest) {
      actions.push(m.can_edit ? '取消改行程' : '允许改行程')
      handlers.push(() => this.togglePerm({ currentTarget: { dataset: { id, k: 'can_edit' } } }))
      actions.push(m.can_bill ? '取消记账' : '允许记账')
      handlers.push(() => this.togglePerm({ currentTarget: { dataset: { id, k: 'can_bill' } } }))
    }
    if (m.role !== 1) {
      actions.push('移除成员')
      handlers.push(() => this.removeMember({ currentTarget: { dataset: { id } } }))
    }
    if (!actions.length) return
    wx.showActionSheet({
      itemList: actions,
      success: (r) => {
        const fn = handlers[r.tapIndex]
        if (!fn) return
        // 连续 ActionSheet 需错开一帧，否则会被系统吃掉
        setTimeout(fn, 50)
      },
    })
  },
  copyCode() {
    wx.setClipboardData({ data: this.data.trip.invite_code })
  },
  addCompanion() {
    if (this.data.trip.role !== 1) return
    wx.showModal({
      title: '添加随行成员',
      editable: true,
      placeholderText: '昵称，如 小朋友',
      success: async (r) => {
        if (!r.confirm) return
        const nickname = (r.content || '').trim()
        if (!nickname) {
          wx.showToast({ title: '请填写昵称', icon: 'none' })
          return
        }
        const groups = [...new Set((this.data.members || []).map((m) => m.group_name).filter(Boolean))]
        let group_name = ''
        if (groups.length) {
          const pick = await new Promise((resolve) => {
            wx.showActionSheet({
              itemList: groups.concat(['新建团体', '各自单独结算']),
              success: (a) => resolve(a.tapIndex),
              fail: () => resolve(-1),
            })
          })
          if (pick < 0) return
          if (pick < groups.length) group_name = groups[pick]
          else if (pick === groups.length) {
            const g = await new Promise((resolve) => {
              wx.showModal({
                title: '团体名称',
                editable: true,
                placeholderText: '如 我这边',
                success: (x) => resolve(x.confirm ? (x.content || '').trim() : ''),
              })
            })
            if (!g) {
              wx.showToast({ title: '未填写团体', icon: 'none' })
              return
            }
            group_name = g
          }
        } else {
          const g = await new Promise((resolve) => {
            wx.showModal({
              title: '归入哪个团体？',
              editable: true,
              placeholderText: '如 我这边（可留空）',
              success: (x) => resolve(x.confirm ? (x.content || '').trim() : null),
            })
          })
          if (g === null) return
          group_name = g
        }
        await api.travelCompanion({
          travel_id: this.data.id,
          nickname,
          group_name: group_name || undefined,
        })
        this.loadMembers()
      },
    })
  },
  editGroup(e) {
    if (this.data.trip.role !== 1) return
    const id = Number(e.currentTarget.dataset.id)
    const m = (this.data.members || []).find((i) => i.user_id === id)
    if (!m) return
    const groups = [...new Set((this.data.members || []).map((x) => x.group_name).filter(Boolean))]
    const extras = ['新建团体', '改为单独结算']
    wx.showActionSheet({
      itemList: groups.concat(extras),
      success: async (r) => {
        let group_name = null
        if (r.tapIndex < groups.length) group_name = groups[r.tapIndex]
        else if (r.tapIndex === groups.length) {
          const g = await new Promise((resolve) => {
            wx.showModal({
              title: '团体名称',
              editable: true,
              placeholderText: '如 朋友这边',
              success: (x) => resolve(x.confirm ? (x.content || '').trim() : ''),
            })
          })
          if (!g) return
          group_name = g
        } else {
          group_name = ''
        }
        await api.travelGroup({
          travel_id: this.data.id,
          user_id: id,
          group_name,
        })
        this.loadMembers()
      },
    })
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
  onTravelMgmt() {
    const trip = this.data.trip || {}
    if (trip.is_sample || Number(trip.status) === 2) return
    const items = []
    const actions = []
    if (this.data.canEdit) {
      items.push('排行程')
      actions.push('plan')
    }
    if (trip.role === 1) {
      items.push('修改旅途')
      actions.push('edit')
    }
    if (this.data.isCreator) {
      items.push('归档行程')
      actions.push('archive')
    }
    if (!items.length) return
    wx.showActionSheet({
      itemList: items,
      success: (r) => {
        const action = actions[r.tapIndex]
        setTimeout(() => {
          if (action === 'plan') this.enterEdit()
          else if (action === 'edit') this.goEditTravel()
          else if (action === 'archive') this.archive()
        }, 80)
      },
    })
  },
  goEditTravel() {
    if (this.data.trip.is_lock) {
      wx.showToast({ title: '已锁定，不可修改', icon: 'none' })
      return
    }
    wx.navigateTo({ url: `/pages/travel/edit?id=${this.data.id}` })
  },
  async archive() {
    if (!this.data.isCreator) {
      wx.showToast({ title: '仅创建人可归档', icon: 'none' })
      return
    }
    const ok = await new Promise((resolve) => {
      wx.showModal({
        title: '归档旅途',
        content: '归档后不可再改行程和账单，将移入历史旅途，并可查看智能分账。确定归档？',
        success: (r) => resolve(r.confirm),
      })
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
