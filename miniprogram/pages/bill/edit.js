const { api } = require('../../utils/api')
const { COST_TYPES, costLabel } = require('../../utils/constants')

function today() {
  const d = new Date()
  const m = `${d.getMonth() + 1}`.padStart(2, '0')
  const day = `${d.getDate()}`.padStart(2, '0')
  return `${d.getFullYear()}-${m}-${day}`
}

Page({
  data: {
    travel_id: 0,
    id: null,
    bill_type: 1,
    bill_name: '',
    amount: '',
    cost_type: 'other',
    costLabel: '其他',
    consume_date: today(),
    pay_user_id: 0,
    payName: '',
    day_plan_id: null,
    planName: '',
    visible_all: false,
    remark: '',
    members: [],
    plans: [],
    costTypes: COST_TYPES,
  },
  async onLoad(q) {
    const travel_id = Number(q.travel_id)
    const trip = await api.travelDetail(travel_id)
    if (!trip.can_bill) {
      wx.showToast({ title: '没有记账权限', icon: 'none' })
      setTimeout(() => wx.navigateBack(), 500)
      return
    }
    const user = getApp().globalData.user || wx.getStorageSync('user') || {}
    const members = await api.travelMember(travel_id)
    const planData = await api.planList(travel_id)
    const plans = (planData.days || []).flatMap((d) => (d.plans || []).map((p) => ({ ...p, label: `D${p.day_num} ${p.place_name}` })))
    let day_plan_id = q.day_plan_id ? Number(q.day_plan_id) : null
    let planName = ''
    let cost_type = 'other'
    if (day_plan_id) {
      const p = plans.find((i) => i.id === day_plan_id)
      if (p) {
        planName = p.label
        cost_type = ['sight', 'hotel', 'food', 'gas', 'transport'].includes(p.point_type) ? p.point_type : 'other'
      }
    }
    this.setData({
      travel_id,
      id: q.id ? Number(q.id) : null,
      members: members.map((m) => ({ ...m, checked: true })),
      plans,
      pay_user_id: user.id,
      payName: user.nickname,
      day_plan_id,
      planName,
      cost_type,
      costLabel: costLabel(cost_type),
      visible_all: !!user.default_bill_visible,
    })
    if (q.id) {
      const bills = await api.billList(travel_id)
      const b = (bills || []).find((i) => i.id === Number(q.id))
      if (b) {
        const shareIds = (b.shares || []).map((s) => s.user_id)
        this.setData({
          bill_type: b.bill_type,
          bill_name: b.bill_name,
          amount: String(b.amount),
          cost_type: b.cost_type,
          costLabel: costLabel(b.cost_type),
          consume_date: (b.consume_time || '').slice(0, 10) || today(),
          pay_user_id: b.pay_user_id,
          payName: b.pay_nickname,
          day_plan_id: b.day_plan_id,
          planName: b.plan_place_name || '',
          visible_all: b.visible_all,
          remark: b.remark || '',
          members: members.map((m) => ({ ...m, checked: !shareIds.length || shareIds.includes(m.user_id) })),
        })
      }
    }
  },
  setType(e) { this.setData({ bill_type: Number(e.currentTarget.dataset.v) }) },
  setCost(e) {
    const cost_type = e.currentTarget.dataset.v
    this.setData({ cost_type, costLabel: costLabel(cost_type) })
  },
  onName(e) { this.setData({ bill_name: e.detail.value }) },
  onAmount(e) { this.setData({ amount: e.detail.value }) },
  onRemark(e) { this.setData({ remark: e.detail.value }) },
  onDate(e) { this.setData({ consume_date: e.detail.value }) },
  onVisible(e) { this.setData({ visible_all: e.detail.value }) },
  pickPayer() {
    wx.showActionSheet({
      itemList: this.data.members.map((m) => m.nickname),
      success: (r) => {
        const m = this.data.members[r.tapIndex]
        this.setData({ pay_user_id: m.user_id, payName: m.nickname })
      },
    })
  },
  clearPlan() {
    this.setData({ day_plan_id: null, planName: '' })
  },
  setPlan(e) {
    const id = e.currentTarget.dataset.id
    const p = this.data.plans.find((i) => i.id === id)
    this.setData({ day_plan_id: id, planName: p ? p.label : '' })
  },
  toggleShare(e) {
    const id = e.currentTarget.dataset.id
    const members = this.data.members.map((m) => (m.user_id === id ? { ...m, checked: !m.checked } : m))
    this.setData({ members })
  },
  async submit() {
    const d = this.data
    if (!d.bill_name || !d.amount) {
      wx.showToast({ title: '请填写名称和金额', icon: 'none' })
      return
    }
    const share_user_ids = d.members.filter((m) => m.checked).map((m) => m.user_id)
    wx.showLoading({ title: '保存中' })
    try {
      await api.billSave({
        id: d.id,
        travel_id: d.travel_id,
        day_plan_id: d.day_plan_id || null,
        bill_name: d.bill_name,
        amount: Number(d.amount),
        bill_type: d.bill_type,
        cost_type: d.cost_type,
        pay_user_id: d.pay_user_id,
        consume_time: `${d.consume_date} 12:00:00`,
        visible_all: d.visible_all,
        share_user_ids: d.bill_type === 1 ? share_user_ids : [],
        remark: d.remark,
      })
      wx.navigateBack()
    } finally {
      wx.hideLoading()
    }
  },
  async remove() {
    const ok = await new Promise((resolve) => {
      wx.showModal({ title: '删除账单', content: '确定删除？', success: (r) => resolve(r.confirm) })
    })
    if (!ok) return
    await api.billDel(this.data.id)
    wx.navigateBack()
  },
})
