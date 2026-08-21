const { api } = require('../../utils/api')

Page({
  data: {
    travel_id: 0,
    groups: [],
    groupTransfers: [],
    innerGroups: [],
  },
  async onLoad(q) {
    this.setData({ travel_id: Number(q.travel_id) })
    this.load()
  },
  async load() {
    const data = await api.settleCalc(this.data.travel_id)
    const groups = data.groups || []
    const innerGroups = groups.filter((g) => g.is_party && (g.member_count || 0) > 1)
    this.setData({
      groups,
      groupTransfers: data.group_transfers || [],
      innerGroups,
    })
  },
})
