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
        arrowLine: true,
      }
    })
    .filter(Boolean)
}

function toMarkers(points) {
  return (points || [])
    .filter((p) => p.latitude && p.longitude)
    .map((p, i) => ({
      id: p.id,
      latitude: Number(p.latitude),
      longitude: Number(p.longitude),
      width: 22,
      height: 22,
      callout: {
        content: `${i + 1}. ${p.place_name}`,
        display: 'ALWAYS',
        padding: 6,
        borderRadius: 8,
        fontSize: 12,
        color: '#1A1A1A',
        bgColor: '#FFFFFF',
      },
    }))
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
  openMap,
}
