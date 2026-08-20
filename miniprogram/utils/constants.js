const POINT_TYPES = [
  { value: 'sight', label: '景点', color: '#7C3AED' },
  { value: 'hotel', label: '住宿', color: '#0052D9' },
  { value: 'food', label: '餐饮', color: '#E37318' },
  { value: 'via', label: '途经点', color: '#00A870' },
  { value: 'gas', label: '加油点', color: '#008858' },
  { value: 'transport', label: '交通', color: '#5E5E5E' },
]

const TRAFFIC_TYPES = [
  { value: 'walk', label: '步行' },
  { value: 'drive', label: '自驾' },
  { value: 'highspeed', label: '高铁' },
  { value: 'train', label: '火车' },
  { value: 'plane', label: '飞机' },
  { value: 'bus', label: '大巴' },
]

const COST_TYPES = [
  { value: 'sight', label: '门票' },
  { value: 'hotel', label: '住宿' },
  { value: 'food', label: '餐饮' },
  { value: 'shop', label: '购物' },
  { value: 'gas', label: '油费/电费' },
  { value: 'transport', label: '交通' },
  { value: 'other', label: '其他' },
]

const PRESET_PLACES = [
  { name: '成都双流机场', longitude: 103.947, latitude: 30.578, point_type: 'transport' },
  { name: '宽窄巷子', longitude: 104.054, latitude: 30.672, point_type: 'food' },
  { name: '春熙路', longitude: 104.082, latitude: 30.657, point_type: 'sight' },
  { name: '都江堰景区', longitude: 103.61, latitude: 31.004, point_type: 'sight' },
  { name: '青城山', longitude: 103.57, latitude: 30.9, point_type: 'sight' },
  { name: '四姑娘山双桥沟', longitude: 102.9, latitude: 31.108, point_type: 'sight' },
  { name: '日隆镇', longitude: 102.828, latitude: 30.99, point_type: 'hotel' },
  { name: '甲居藏寨', longitude: 101.963, latitude: 30.951, point_type: 'sight' },
  { name: '丹巴县城', longitude: 101.891, latitude: 30.877, point_type: 'hotel' },
  { name: '新都桥', longitude: 101.491, latitude: 30.043, point_type: 'sight' },
  { name: '泸定桥', longitude: 102.234, latitude: 29.914, point_type: 'sight' },
]

function pointMeta(type) {
  return POINT_TYPES.find((i) => i.value === type) || POINT_TYPES[0]
}

function costLabel(type) {
  return (COST_TYPES.find((i) => i.value === type) || {}).label || type
}

function trafficLabel(type) {
  return (TRAFFIC_TYPES.find((i) => i.value === type) || {}).label || type || ''
}

function formatLegHint(distanceM, durationS) {
  const m = Number(distanceM) || 0
  const s = Number(durationS) || 0
  if (m <= 0 && s <= 0) return ''
  let dist = ''
  if (m > 0) {
    dist = m >= 1000 ? `${(m / 1000).toFixed(m >= 10000 ? 0 : 1)}km` : `${Math.round(m)}m`
  }
  let time = ''
  if (s > 0) {
    if (s < 90) time = '约1分钟'
    else if (s < 3600) time = `约${Math.round(s / 60)}分钟`
    else {
      const h = Math.floor(s / 3600)
      const min = Math.round((s % 3600) / 60)
      time = min ? `约${h}小时${min}分` : `约${h}小时`
    }
  }
  if (dist && time) return `${dist} · ${time}`
  return dist || time
}

function legDurationSeconds(p, line) {
  if (p && Number(p.traffic_duration) > 0) return Number(p.traffic_duration) * 60
  if (p && Number(p.next_duration_s) > 0) return Number(p.next_duration_s)
  if (line && Number(line.duration_s) > 0) return Number(line.duration_s)
  return 0
}

function withLegHints(points, lines) {
  const map = {}
  ;(lines || []).forEach((l) => {
    map[l.from_id] = l
  })
  return (points || []).map((p) => {
    const l = map[p.id] || {}
    const distance = p.next_distance_m || l.distance_m
    return { ...p, legHint: formatLegHint(distance, legDurationSeconds(p, l)) }
  })
}

function openMap(item) {
  const latitude = Number(item && item.latitude)
  const longitude = Number(item && item.longitude)
  if (!latitude || !longitude) {
    wx.showToast({ title: '该点还没有坐标', icon: 'none' })
    return
  }
  wx.openLocation({
    latitude,
    longitude,
    name: item.place_name || item.name || '行程地点',
    address: item.place_name || '',
    scale: 16,
  })
}

function linesToPolyline(lines, points) {
  return (lines || [])
    .map((l) => {
      let pts = (l.points || []).filter((p) => p.latitude && p.longitude)
      if (pts.length < 2 && points) {
        const a = points.find((p) => p.id === l.from_id)
        const b = points.find((p) => p.id === l.to_id)
        if (a && b && a.latitude && b.latitude) {
          pts = [
            { latitude: a.latitude, longitude: a.longitude },
            { latitude: b.latitude, longitude: b.longitude },
          ]
        }
      }
      if (pts.length < 2) return null
      const color = l.color && l.color.length === 7 ? `${l.color}FF` : (l.color || '#0052D9FF')
      const dotted = !!l.dotted || l.traffic_type === 'plane' || l.traffic_type === 'highspeed' || l.traffic_type === 'train' || l.traffic_type === 'walk'
      return {
        points: pts,
        color,
        width: dotted ? 4 : 6,
        dottedLine: dotted,
        // arrowLine 在部分安卓机会导致地图区域持续闪烁
        arrowLine: false,
      }
    })
    .filter(Boolean)
}

/** 坐标几乎相同的点，仅在地图展示时轻微错开（不改真实坐标） */
function spreadOverlappingPoints(points) {
  const list = (points || [])
    .filter((p) => p.latitude && p.longitude)
    .map((p) => ({ ...p }))
  const groups = {}
  list.forEach((p, i) => {
    const key = `${Number(p.latitude).toFixed(5)},${Number(p.longitude).toFixed(5)}`
    if (!groups[key]) groups[key] = []
    groups[key].push(i)
  })
  Object.keys(groups).forEach((key) => {
    const idxs = groups[key]
    if (idxs.length < 2) return
    const baseLat = Number(list[idxs[0]].latitude)
    const baseLng = Number(list[idxs[0]].longitude)
    const cos = Math.max(0.2, Math.cos((baseLat * Math.PI) / 180))
    idxs.forEach((idx, j) => {
      const ring = Math.floor(j / 6)
      const pos = j % 6
      const inRing = Math.min(6, idxs.length - ring * 6)
      const angle = (2 * Math.PI * pos) / inRing - Math.PI / 2
      // ~30m 起，多点再扩一圈，避免针和气泡叠死
      const radius = 0.00032 * (ring + 1)
      list[idx].latitude = baseLat + radius * Math.cos(angle)
      list[idx].longitude = baseLng + (radius * Math.sin(angle)) / cos
      list[idx]._spread = true
    })
  })
  return list
}

function toMarkers(points, opts) {
  const markStart = !!(opts && opts.markStart)
  return spreadOverlappingPoints(points).map((p, i) => {
    const name = String(p.place_name || p.name || '').trim()
    const short = name.length > 10 ? `${name.slice(0, 10)}…` : name
    const isStart = !!(p.isStart || (markStart && i === 0))
    let content = short ? `${i + 1}. ${short}` : String(i + 1)
    if (isStart) content = short ? `起点 · ${short}` : '起点'
    return {
      id: Number(p.id) || i + 1,
      latitude: Number(p.latitude),
      longitude: Number(p.longitude),
      width: isStart ? 28 : 22,
      height: isStart ? 42 : 34,
      anchor: { x: 0.5, y: 1 },
      zIndex: isStart ? 99 : i + 1,
      callout: {
        content,
        display: 'ALWAYS',
        padding: 6,
        borderRadius: 8,
        fontSize: isStart ? 12 : 11,
        color: isStart ? '#ffffff' : '#2f3d36',
        bgColor: isStart ? '#6f9b88' : '#ffffff',
        borderWidth: 1,
        borderColor: isStart ? '#5d8a76' : '#d7e4dc',
        textAlign: 'center',
      },
    }
  })
}

/** includePoints 用：把起点往视野里收一点，避免贴边/气泡被裁掉像「没点」 */
function fitPointsForMap(points) {
  const pts = (points || [])
    .filter((p) => p.latitude && p.longitude)
    .map((p) => ({
      latitude: Number(p.latitude),
      longitude: Number(p.longitude),
    }))
  if (!pts.length) return pts
  if (pts.length === 1) {
    const s = pts[0]
    const pad = 0.08
    return [
      s,
      { latitude: s.latitude + pad, longitude: s.longitude },
      { latitude: s.latitude - pad, longitude: s.longitude },
      { latitude: s.latitude, longitude: s.longitude + pad },
      { latitude: s.latitude, longitude: s.longitude - pad },
    ]
  }
  const start = pts[0]
  let latSum = 0
  let lngSum = 0
  pts.forEach((p) => {
    latSum += p.latitude
    lngSum += p.longitude
  })
  const cLat = latSum / pts.length
  const cLng = lngSum / pts.length
  const dLat = start.latitude - cLat
  const dLng = start.longitude - cLng
  const dist = Math.sqrt(dLat * dLat + dLng * dLng) || 0.05
  // 起点外侧再扩一截，保证第一天「远起点」也落在框内
  pts.push({
    latitude: start.latitude + dLat * 0.35,
    longitude: start.longitude + dLng * 0.35,
  })
  const pad = Math.max(0.06, Math.min(dist * 0.12, 0.35))
  pts.push(
    { latitude: start.latitude + pad, longitude: start.longitude },
    { latitude: start.latitude - pad, longitude: start.longitude },
    { latitude: start.latitude, longitude: start.longitude + pad },
    { latitude: start.latitude, longitude: start.longitude - pad },
  )
  return pts
}

/** 当天列表点 + 跨天出发起点（有坐标才拼上） */
function withDayStart(plans, startFrom) {
  const list = (plans || []).slice()
  if (!startFrom || !startFrom.latitude || !startFrom.longitude) return list
  const first = list[0]
  if (first && String(first.place_name || '').trim() === String(startFrom.place_name || '').trim()) {
    return list
  }
  return [
    {
      id: startFrom.id || -1,
      place_name: startFrom.place_name,
      latitude: startFrom.latitude,
      longitude: startFrom.longitude,
      point_type: startFrom.point_type || 'hotel',
      isStart: true,
    },
    ...list,
  ]
}

module.exports = {
  POINT_TYPES,
  TRAFFIC_TYPES,
  COST_TYPES,
  PRESET_PLACES,
  pointMeta,
  costLabel,
  trafficLabel,
  formatLegHint,
  legDurationSeconds,
  withLegHints,
  linesToPolyline,
  toMarkers,
  withDayStart,
  fitPointsForMap,
  openMap,
}
