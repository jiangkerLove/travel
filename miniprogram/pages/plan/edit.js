const { api } = require('../../utils/api')
const { POINT_TYPES, TRAFFIC_TYPES } = require('../../utils/constants')

function pin(lat, lng) {
  if (!lat || !lng) return []
  return [{
    id: 1,
    latitude: Number(lat),
    longitude: Number(lng),
    // 默认水滴偏高，正方形会被压扁
    width: 24,
    height: 36,
    anchor: { x: 0.5, y: 1 },
  }]
}

Page({
  data: {
    travel_id: 0,
    id: null,
    after_id: null,
    day_num: 1,
    dayOptions: [],
    point_type: 'sight',
    traffic_type: 'drive',
    pois: [],
    searching: false,
    emptyHint: false,
    showSearch: true,
    searchFocus: true,
    picked: false,
    mapShown: false,
    mapLat: 30.67,
    mapLng: 104.06,
    markers: [],
    place_name: '',
    place_address: '',
    longitude: null,
    latitude: null,
    remark: '',
    pointTypes: POINT_TYPES,
    trafficTypes: TRAFFIC_TYPES,
    isEdit: false,
  },
  async onLoad(q) {
    this._keyword = ''
    this._seq = 0
    this._mapInited = false
    this._booted = false
    this.setData({
      travel_id: Number(q.travel_id),
      day_num: Number(q.day_num || 1),
      id: q.id ? Number(q.id) : null,
      after_id: q.after_id ? Number(q.after_id) : null,
      isEdit: !!q.id,
      point_type: 'sight',
      traffic_type: 'drive',
      pointTypes: POINT_TYPES,
      trafficTypes: TRAFFIC_TYPES,
    })
    wx.setNavigationBarTitle({
      title: q.id ? '编辑地点' : '添加地点',
    })
    wx.getLocation({
      type: 'gcj02',
      success: (r) => {
        this._aroundLng = r.longitude
        this._aroundLat = r.latitude
      },
    })

    // 优先用旅途页已有数据，避免进页再刷接口闪一下
    try {
      const channel = this.getOpenerEventChannel && this.getOpenerEventChannel()
      if (channel && channel.on) {
        channel.on('init', (payload) => {
          if (this._booted) return
          this.bootFromParent(payload || {}, q)
        })
      }
    } catch (e) { /* ignore */ }

    // 无 opener 数据时再兜底请求
    setTimeout(() => {
      if (!this._booted) this.bootFromApi(q)
    }, 80)
  },
  denyReadonly() {
    wx.showToast({ title: '示例/归档旅途仅可查看', icon: 'none' })
    setTimeout(() => wx.navigateBack(), 500)
  },
  applyDayOptions(days) {
    const dayOptions = (days || []).map((d) => ({
      day_num: d.day_num,
      label: `D${d.day_num}`,
      date: d.shortDate || (d.date || '').slice(5),
    }))
    this.setData({ dayOptions })
    return dayOptions
  },
  applyPlan(p) {
    if (!p) return
    this._keyword = p.place_name || ''
    this._mapInited = true
    this._origDay = p.day_num
    const pointType = p.point_type === 'via' ? 'sight' : (p.point_type || 'sight')
    const trafficType = TRAFFIC_TYPES.some((t) => t.value === p.traffic_type)
      ? p.traffic_type
      : 'drive'
    this.setData({
      day_num: p.day_num,
      point_type: POINT_TYPES.some((t) => t.value === pointType) ? pointType : 'sight',
      traffic_type: trafficType,
      showSearch: false,
      searchFocus: false,
      picked: true,
      mapShown: true,
      place_name: p.place_name,
      place_address: '',
      longitude: p.longitude,
      latitude: p.latitude,
      mapLat: Number(p.latitude) || 30.67,
      mapLng: Number(p.longitude) || 104.06,
      markers: pin(p.latitude, p.longitude),
      remark: p.remark || '',
    })
  },
  bootFromParent(payload, q) {
    const trip = payload.trip || {}
    if (trip.id && (!trip.can_edit || trip.is_sample || Number(trip.status) === 2)) {
      this._booted = true
      this.denyReadonly()
      return
    }
    this._booted = true
    const days = payload.days || []
    this.applyDayOptions(days)
    let plan = payload.plan
    if (!plan && q.id) {
      plan = days.flatMap((d) => d.plans || []).find((i) => i.id === Number(q.id))
    }
    if (plan) this.applyPlan(plan)
    // 先展示带入数据，再静默刷新天数列表
    this.refreshDaysQuiet()
  },
  async refreshDaysQuiet() {
    try {
      const data = await api.planList(this.data.travel_id, null, false)
      if (!data || !data.days) return
      this.applyDayOptions(data.days)
    } catch (e) { /* 静默失败，沿用带入数据 */ }
  },
  async bootFromApi(q) {
    if (this._booted) return
    this._booted = true
    try {
      const trip = await api.travelDetail(this.data.travel_id)
      if (!trip.can_edit || trip.is_sample || Number(trip.status) === 2) {
        this.denyReadonly()
        return
      }
      const data = await api.planList(this.data.travel_id, null, false)
      const days = data.days || []
      this.applyDayOptions(days)
      if (q.id) {
        const p = days.flatMap((d) => d.plans || []).find((i) => i.id === Number(q.id))
        if (p) this.applyPlan(p)
      }
    } catch (e) {
      wx.showToast({ title: '加载失败', icon: 'none' })
    }
  },
  onUnload() {
    clearTimeout(this._timer)
  },
  setDay(e) {
    this.setData({ day_num: Number(e.currentTarget.dataset.v) })
  },
  stopSearch() {
    clearTimeout(this._timer)
    this._seq += 1
  },
  openSearch() {
    this.stopSearch()
    this.setData({
      showSearch: true,
      searchFocus: false,
      pois: [],
      searching: false,
      emptyHint: false,
    })
    wx.nextTick(() => this.setData({ searchFocus: true }))
  },
  closeSearch() {
    if (!this.data.picked) return
    this.stopSearch()
    this.setData({
      showSearch: false,
      searchFocus: false,
      pois: [],
      searching: false,
      emptyHint: false,
    })
  },
  onKeyword(e) {
    this._keyword = e.detail.value
    clearTimeout(this._timer)
    this._timer = setTimeout(() => this.search(this._keyword), 450)
  },
  onConfirm(e) {
    this._keyword = e.detail.value
    clearTimeout(this._timer)
    this.search(this._keyword)
  },
  async search(keyword) {
    const q = String(keyword || '').trim()
    if (q.length < 2) {
      this.setData({ pois: [], searching: false, emptyHint: false })
      return
    }
    const seq = ++this._seq
    this.setData({ searching: true, emptyHint: false })
    try {
      const pois = (await api.mapSearch(q, this._aroundLng, this._aroundLat)) || []
      if (seq !== this._seq) return
      this.setData({
        pois,
        searching: false,
        emptyHint: !pois.length,
      })
    } catch (err) {
      if (seq !== this._seq) return
      this.setData({ searching: false, emptyHint: true, pois: [] })
    }
  },
  pickPoi(e) {
    const i = Number(e.currentTarget.dataset.i)
    const poi = this.data.pois[i]
    if (!poi) return
    this.stopSearch()
    wx.hideKeyboard()
    const lat = Number(poi.latitude)
    const lng = Number(poi.longitude)
    if (!lat || !lng) {
      wx.showToast({ title: '这个地址没有坐标', icon: 'none' })
      return
    }
    const first = !this._mapInited
    const patch = {
      pois: [],
      searching: false,
      emptyHint: false,
      showSearch: false,
      searchFocus: false,
      picked: true,
      place_name: poi.name,
      place_address: poi.address || '',
      latitude: lat,
      longitude: lng,
    }
    if (first) {
      this._mapInited = true
      this.setData({
        ...patch,
        mapShown: true,
        mapLat: lat,
        mapLng: lng,
        markers: pin(lat, lng),
      })
      return
    }
    this.setData(patch)
    this.movePin(lat, lng)
  },
  movePin(lat, lng) {
    const ctx = wx.createMapContext('pickMap', this)
    if (this.data.markers.length && ctx && ctx.translateMarker) {
      ctx.translateMarker({
        markerId: 1,
        destination: { latitude: lat, longitude: lng },
        autoRotate: false,
        rotate: 0,
        duration: 0,
      })
    } else {
      this.setData({ markers: pin(lat, lng) })
    }
    wx.nextTick(() => {
      if (ctx && ctx.includePoints) {
        ctx.includePoints({
          padding: [80, 80, 80, 80],
          points: [{ latitude: lat, longitude: lng }],
        })
      }
    })
  },
  setType(e) { this.setData({ point_type: e.currentTarget.dataset.v }) },
  setTraffic(e) { this.setData({ traffic_type: e.currentTarget.dataset.v }) },
  onRemark(e) { this.setData({ remark: e.detail.value }) },
  async submit() {
    const d = this.data
    if (!d.picked || !d.latitude || !d.place_name) {
      wx.showToast({ title: '请先选择地点', icon: 'none' })
      return
    }
    wx.showLoading({ title: '保存中' })
    try {
      await api.planSave({
        id: d.id,
        travel_id: d.travel_id,
        day_num: d.day_num,
        point_type: d.point_type,
        place_name: d.place_name,
        longitude: d.longitude,
        latitude: d.latitude,
        traffic_type: d.traffic_type || 'drive',
        traffic_duration: null,
        remark: d.remark,
        after_id: d.after_id,
      })
      if (d.id && this._origDay && this._origDay !== d.day_num) {
        await api.planMove({
          travel_id: d.travel_id,
          id: d.id,
          day_num: d.day_num,
        })
      }
      wx.navigateBack()
    } finally {
      wx.hideLoading()
    }
  },
})
