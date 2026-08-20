const { api } = require('../../utils/api')
const { POINT_TYPES } = require('../../utils/constants')

function pin(lat, lng) {
  if (!lat || !lng) return []
  return [{
    id: 1,
    latitude: Number(lat),
    longitude: Number(lng),
    width: 28,
    height: 28,
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
    pointTypes: POINT_TYPES.filter((t) => t.value !== 'via'),
    isEdit: false,
    isVia: false,
  },
  async onLoad(q) {
    const isVia = q.via === '1'
    this._keyword = ''
    this._seq = 0
    this._mapInited = false
    this.setData({
      travel_id: Number(q.travel_id),
      day_num: Number(q.day_num || 1),
      id: q.id ? Number(q.id) : null,
      after_id: q.after_id ? Number(q.after_id) : null,
      isEdit: !!q.id,
      isVia,
      point_type: isVia ? 'via' : 'sight',
      pointTypes: isVia
        ? POINT_TYPES.filter((t) => t.value === 'via')
        : POINT_TYPES.filter((t) => t.value !== 'via'),
    })
    wx.setNavigationBarTitle({
      title: q.id ? '编辑地点' : (isVia ? '添加途经点' : '添加地点'),
    })
    const trip = await api.travelDetail(this.data.travel_id)
    if (!trip.can_edit || trip.is_sample || Number(trip.status) === 2) {
      wx.showToast({ title: '示例/归档旅途仅可查看', icon: 'none' })
      setTimeout(() => wx.navigateBack(), 500)
      return
    }
    wx.getLocation({
      type: 'gcj02',
      success: (r) => {
        this._aroundLng = r.longitude
        this._aroundLat = r.latitude
      },
    })
    const data = await api.planList(this.data.travel_id, null, false)
    const days = data.days || []
    const dayOptions = days.map((d) => ({
      day_num: d.day_num,
      label: `D${d.day_num}`,
      date: (d.date || '').slice(5),
    }))
    this.setData({ dayOptions })
    if (q.id) {
      const all = days.flatMap((d) => d.plans || [])
      const p = all.find((i) => i.id === Number(q.id))
      if (p) {
        this._keyword = p.place_name || ''
        this._mapInited = true
        this._origDay = p.day_num
        this.setData({
          day_num: p.day_num,
          point_type: p.point_type,
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
          isVia: p.point_type === 'via',
          pointTypes: p.point_type === 'via'
            ? POINT_TYPES.filter((t) => t.value === 'via')
            : POINT_TYPES.filter((t) => t.value !== 'via'),
        })
      }
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
        traffic_type: 'drive',
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
  async remove() {
    const id = this.data.id
    if (!id) return
    const ok = await new Promise((resolve) => {
      wx.showModal({ title: '删除', content: '确定删除这个地点？', success: (r) => resolve(r.confirm) })
    })
    if (!ok) return
    wx.showLoading({ title: '删除中' })
    try {
      await api.planDel(id)
      wx.navigateBack()
    } finally {
      wx.hideLoading()
    }
  },
})
