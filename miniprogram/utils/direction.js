function sketchSegment(from, to, traffic) {
  const start = { latitude: Number(from.latitude), longitude: Number(from.longitude) }
  const end = { latitude: Number(to.latitude), longitude: Number(to.longitude) }
  if (traffic !== 'plane') return [start, end]
  const n = 16
  const dLat = end.latitude - start.latitude
  const dLng = end.longitude - start.longitude
  const midLat = ((start.latitude + end.latitude) / 2) * Math.PI / 180
  const cos = Math.max(0.2, Math.cos(midLat))
  let pLng = -dLat / cos
  let pLat = dLng * cos
  const plen = Math.sqrt(pLng * pLng + pLat * pLat) || 1
  pLng /= plen
  pLat /= plen
  const mag = 0.18 * Math.sqrt(dLat * dLat + (dLng * cos) ** 2)
  const pts = []
  for (let i = 0; i <= n; i++) {
    const t = i / n
    const ease = Math.sin(Math.PI * t)
    pts.push({
      latitude: start.latitude + dLat * t + pLat * mag * ease,
      longitude: start.longitude + dLng * t + pLng * mag * ease,
    })
  }
  return pts
}

function fillLineRoutes(lines, points) {
  const map = {}
  ;(points || []).forEach((p) => {
    map[p.id] = p
  })
  const out = (lines || []).map((line) => {
    const a = map[line.from_id]
    const b = map[line.to_id]
    if (!a || !b || !a.latitude || !b.latitude) return line
    if ((line.points || []).length >= 2) return line
    return {
      ...line,
      points: sketchSegment(a, b, line.traffic_type),
    }
  })
  const fromNav = out.some((l) => l.from_nav)
  return { lines: out, fromNav }
}

module.exports = { fillLineRoutes, sketchSegment }
