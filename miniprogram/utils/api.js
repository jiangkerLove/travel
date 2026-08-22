const { request } = require('./request')

const api = {
  login: (data) => request({ url: '/api/user/login', method: 'POST', data, skipAuth: true, quiet: true }),
  userInfo: () => request({ url: '/api/user/info' }),
  updateUser: (data) => request({ url: '/api/user/info', method: 'POST', data }),
  seed: () => request({ url: '/api/dev/seed', method: 'POST', data: {} }),

  travelList: (archived = false) => request({ url: `/api/travel/list?archived=${archived}` }),
  travelDetail: (id) => request({ url: `/api/travel/detail?id=${id}` }),
  travelCreate: (data) => request({ url: '/api/travel/create', method: 'POST', data }),
  travelUpdate: (data) => request({ url: '/api/travel/update', method: 'POST', data }),
  travelJoin: (invite_code) => request({ url: '/api/travel/join', method: 'POST', data: { invite_code } }),
  travelMember: (travel_id) => request({ url: `/api/travel/member?travel_id=${travel_id}` }),
  travelLock: (data) => request({ url: '/api/travel/lock', method: 'POST', data }),
  travelQuit: (travel_id) => request({ url: '/api/travel/quit', method: 'POST', data: { travel_id } }),
  travelArchive: (travel_id) => request({ url: '/api/travel/archive', method: 'POST', data: { travel_id } }),
  travelRemove: (data) => request({ url: '/api/travel/remove', method: 'POST', data }),
  travelPerm: (data) => request({ url: '/api/travel/perm', method: 'POST', data }),
  travelCompanion: (data) => request({ url: '/api/travel/companion', method: 'POST', data }),
  travelGroup: (data) => request({ url: '/api/travel/group', method: 'POST', data }),

  planList: (travel_id, day_num, routes = true) => {
    let url = `/api/plan/list?travel_id=${travel_id}`
    if (day_num) url += `&day_num=${day_num}`
    if (!routes) url += '&routes=0'
    return request({ url })
  },
  planSave: (data) => request({ url: '/api/plan/save', method: 'POST', data }),
  planDel: (id) => request({ url: '/api/plan/del', method: 'POST', data: { id } }),
  planSort: (data) => request({ url: '/api/plan/sort', method: 'POST', data }),
  planMove: (data) => request({ url: '/api/plan/move', method: 'POST', data }),
  planAiDraft: (data) => request({ url: '/api/plan/ai-draft', method: 'POST', data, timeout: 60000 }),
  planAiApply: (data) => request({ url: '/api/plan/ai-apply', method: 'POST', data, timeout: 30000 }),
  mapDay: (travel_id, day_num, fresh = false) => {
    let url = `/api/map/day?travel_id=${travel_id}&day_num=${day_num}`
    if (fresh) url += '&fresh=1'
    return request({ url, timeout: 25000 })
  },
  mapGlobal: (travel_id) => request({ url: `/api/map/global?travel_id=${travel_id}`, timeout: 25000 }),
  mapSearch: (q, lng, lat) => {
    let url = `/api/map/search?q=${encodeURIComponent(q || '')}`
    if (lng && lat) url += `&lng=${lng}&lat=${lat}`
    return request({ url })
  },
  mapRegeo: (lng, lat) => request({ url: `/api/map/regeo?lng=${lng}&lat=${lat}` }),

  billList: (travel_id) => request({ url: `/api/bill/list?travel_id=${travel_id}` }),
  billSave: (data) => request({ url: '/api/bill/save', method: 'POST', data }),
  billDel: (id) => request({ url: '/api/bill/del', method: 'POST', data: { id } }),
  statTotal: (travel_id) => request({ url: `/api/stat/total?travel_id=${travel_id}` }),
  settleCalc: (travel_id) => request({ url: `/api/settle/calc?travel_id=${travel_id}` }),
}

module.exports = { api }
