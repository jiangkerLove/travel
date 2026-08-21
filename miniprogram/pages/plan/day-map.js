const { api } = require('../../utils/api')
const { pointMeta, trafficLabel, linesToPolyline, toMarkers, openMap, withLegHints } = require('../../utils/constants')
const { fillLineRoutes } = require('../../utils/direction')

Page({
  data: { mapLat: 30.67, mapLng: 104.06, markers: [], polyline: [], includePoints: [], points: [], routeHint: '' },
  async onLoad(q) {
    const data = await api.mapDay(Number(q.travel_id), Number(q.day_num))
    const points = (data.points || []).map((p) => ({
      ...p,
      typeLabel: pointMeta(p.point_type).label,
      trafficLabel: trafficLabel(p.traffic_type),
    }))
    const filled = fillLineRoutes(data.lines || [], points)
    const withHints = withLegHints(points, filled.lines)
    const withGeo = withHints.filter((p) => p.latitude && p.longitude)
    const markers = toMarkers(withGeo)
    const polyline = linesToPolyline(filled.lines || [], withHints)
    const includePoints = withGeo.map((p) => ({ latitude: p.latitude, longitude: p.longitude }))
    const c = withGeo[0] || { latitude: 30.67, longitude: 104.06 }
    this.setData({
      points: withHints,
      markers,
      polyline,
      mapLat: c.latitude,
      mapLng: c.longitude,
      routeHint: filled.fromNav
        ? '路书：按高德驾车/步行路线简化后的当日概览'
        : '路书：当日点到点概览，高铁/飞机为估算',
    })
    if (includePoints.length) {
      wx.nextTick(() => {
        const ctx = wx.createMapContext('dayMap', this)
        if (ctx && ctx.includePoints) {
          ctx.includePoints({ padding: [40, 40, 40, 40], points: includePoints })
        }
      })
    }
  },
  onMarker(e) {
    const markerId = e.detail.markerId
    if (Number(markerId) >= 900000000) return
    const p = this.data.points.find((i) => i.id === markerId)
    if (p) openMap(p)
  },
  openPlace(e) {
    const p = this.data.points.find((i) => i.id === e.currentTarget.dataset.id)
    if (p) openMap(p)
  },
})
