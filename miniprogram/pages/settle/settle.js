const { api } = require('../../utils/api')

Page({
  data: { travel_id: 0, users: [], transfers: [], is_lock: false, is_leader: false },
  async onLoad(q) {
    this.setData({ travel_id: Number(q.travel_id) })
    this.load()
  },
  async load() {
    const data = await api.settleCalc(this.data.travel_id)
    this.setData({
      users: data.users || [],
      transfers: data.transfers || [],
      is_lock: data.is_lock,
      is_leader: data.is_leader,
    })
  },
  async toggleLock() {
    await api.travelLock({ travel_id: this.data.travel_id, is_lock: !this.data.is_lock })
    this.load()
  },
})
