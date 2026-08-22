const ILLUST = [
  { key: 'mountain', label: '山岳' },
  { key: 'beach', label: '海滨' },
  { key: 'island', label: '海岛' },
  { key: 'city', label: '城市' },
  { key: 'forest', label: '森林' },
  { key: 'desert', label: '沙漠' },
  { key: 'temple', label: '古迹' },
  { key: 'lake', label: '湖泊' },
  { key: 'snow', label: '雪国' },
  { key: 'balloon', label: '出发' },
]

const DEFAULT_KEY = 'balloon'
const ROUTE_KEY = 'route'

function illustSrc(key) {
  if (key === ROUTE_KEY) return '/assets/illust/route.svg'
  const ok = ILLUST.some((x) => x.key === key)
  return `/assets/illust/${ok ? key : DEFAULT_KEY}.svg`
}

function coverOptions() {
  return [
    { key: ROUTE_KEY, label: '路线图', src: illustSrc(ROUTE_KEY) },
    ...ILLUST.map((x) => ({ ...x, src: illustSrc(x.key) })),
  ]
}

function hashStr(s) {
  let h = 0
  const str = String(s || '')
  for (let i = 0; i < str.length; i++) h = (Math.imul(31, h) + str.charCodeAt(i)) | 0
  return (h >>> 0).toString(36)
}

function cardThumb(t) {
  const cover = t.cover || ROUTE_KEY
  if (cover !== ROUTE_KEY) return illustSrc(cover)
  const svg = t.route_svg || t.routeSvg
  if (!svg) return illustSrc(ROUTE_KEY)
  try {
    const fs = wx.getFileSystemManager()
    const path = `${wx.env.USER_DATA_PATH}/route-${t.id}-${hashStr(svg)}.svg`
    fs.writeFileSync(path, svg, 'utf8')
    return path
  } catch (e) {
    return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`
  }
}

module.exports = {
  ILLUST,
  DEFAULT_KEY,
  ROUTE_KEY,
  illustSrc,
  coverOptions,
  cardThumb,
}
