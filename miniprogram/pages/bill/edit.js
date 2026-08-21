const { api } = require('../../utils/api')
const { COST_TYPES, costLabel } = require('../../utils/constants')

function today() {
  const d = new Date()
  const m = `${d.getMonth() + 1}`.padStart(2, '0')
  const day = `${d.getDate()}`.padStart(2, '0')
  return `${d.getFullYear()}-${m}-${day}`
}

function plansFromDays(days) {
  return (days || []).flatMap((d) => (d.plans || []).map((p) => ({
    ...p,
    label: `D${p.day_num} ${p.place_name}`,
  })))
}

Page({
  data: {
    travel_id: 0,
    id: null,
    bill_name: '',
    amount: '',
    cost_type: 'other',
    costLabel: '其他',
    consume_date: today(),
    pay_user_id: 0,
    payName: '',
    day_plan_id: null,
    planName: '',
    visible_all: true,
    remark: '',
    members: [],
    plans: [],
    costTypes: COST_TYPES,
    allShared: true,
  },
  onLoad(q) {
    this._booted = false
    const travel_id = Number(q.travel_id)
    const user = getApp().globalData.user || wx.getStorageSync('user') || {}
    this.setData({
      travel_id,
      id: q.id ? Number(q.id) : null,
      pay_user_id: user.id,
      payName: user.nickname,
      consume_date: q.consume_date || today(),
      day_plan_id: q.day_plan_id ? Number(q.day_plan_id) : null,
    })

    try {
      const channel = this.getOpenerEventChannel && this.getOpenerEventChannel()
      if (channel && channel.on) {
        channel.on('init', (payload) => {
          if (this._booted) return
          this.bootFromParent(payload || {}, q, user)
        })
      }
    } catch (e) { /* ignore */ }

    setTimeout(() => {
      if (!this._booted) this.bootFromApi(q, user)
    }, 80)
  },
  bootFromParent(payload, q, user) {
    const trip = payload.trip || {}
    if (trip.id && !trip.can_bill) {
      this._booted = true
      wx.showToast({ title: '没有记账权限', icon: 'none' })
      setTimeout(() => wx.navigateBack(), 500)
      return
    }
    this._booted = true
    const members = (payload.members || []).map((m) => ({ ...m, checked: true }))
    const plans = plansFromDays(payload.days)
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
      members,
      allShared: true,
      plans,
      day_plan_id,
      planName,
      cost_type,
      costLabel: costLabel(cost_type),
      visible_all: true,
      pay_user_id: user.id,
      payName: user.nickname,
    })
    if (payload.bill) this.applyBill(payload.bill, members)
    else if (q.id && payload.bills) {
      const b = (payload.bills || []).find((i) => i.id === Number(q.id))
      if (b) this.applyBill(b, members)
    }
    this.refreshQuiet(q, user)
  },
  applyBill(b, members) {
    let shareIds = (b.shares || []).map((s) => s.user_id)
    if (!shareIds.length && b.bill_type === 2) shareIds = [b.pay_user_id]
    const list = members || this.data.members
    const checkedMembers = list.map((m) => ({
      ...m,
      checked: shareIds.length ? shareIds.includes(m.user_id) : true,
    }))
    this.setData({
      bill_name: b.bill_name,
      amount: String(b.amount),
      cost_type: b.cost_type,
      costLabel: costLabel(b.cost_type),
      consume_date: (b.consume_time || '').slice(0, 10) || today(),
      pay_user_id: b.pay_user_id,
      payName: b.pay_nickname,
      day_plan_id: b.day_plan_id,
      planName: b.plan_place_name || '',
      visible_all: b.bill_type === 2 ? !!b.visible_all : (b.visible_all !== false),
      remark: b.remark || '',
      members: checkedMembers,
      allShared: checkedMembers.length > 0 && checkedMembers.every((m) => m.checked),
    })
  },
  async refreshQuiet(q, user) {
    try {
      const [members, planData, bills] = await Promise.all([
        api.travelMember(this.data.travel_id),
        api.planList(this.data.travel_id, null, false),
        q.id ? api.billList(this.data.travel_id) : Promise.resolve(null),
      ])
      const plans = plansFromDays(planData.days)
      const mapped = (members || []).map((m) => {
        const prev = (this.data.members || []).find((x) => x.user_id === m.user_id)
        return { ...m, checked: prev ? !!prev.checked : true }
      })
      const patch = {
        plans,
        members: mapped,
        allShared: mapped.length > 0 && mapped.every((m) => m.checked),
      }
      if (!this.data.pay_user_id && user.id) {
        patch.pay_user_id = user.id
        patch.payName = user.nickname
      }
      this.setData(patch)
      if (q.id && bills) {
        const b = (bills || []).find((i) => i.id === Number(q.id))
        if (b && !this.data.bill_name) this.applyBill(b, mapped)
      }
    } catch (e) { /* 静默 */ }
  },
  async bootFromApi(q, user) {
    if (this._booted) return
    this._booted = true
    try {
      const trip = await api.travelDetail(this.data.travel_id)
      if (!trip.can_bill) {
        wx.showToast({ title: '没有记账权限', icon: 'none' })
        setTimeout(() => wx.navigateBack(), 500)
        return
      }
      const [members, planData] = await Promise.all([
        api.travelMember(this.data.travel_id),
        api.planList(this.data.travel_id, null, false),
      ])
      const plans = plansFromDays(planData.days)
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
      const mapped = (members || []).map((m) => ({ ...m, checked: true }))
      this.setData({
        members: mapped,
        allShared: true,
        plans,
        day_plan_id,
        planName,
        cost_type,
        costLabel: costLabel(cost_type),
        visible_all: true,
        pay_user_id: user.id,
        payName: user.nickname,
      })
      if (q.id) {
        const bills = await api.billList(this.data.travel_id)
        const b = (bills || []).find((i) => i.id === Number(q.id))
        if (b) this.applyBill(b, mapped)
      }
    } catch (e) {
      wx.showToast({ title: '加载失败', icon: 'none' })
    }
  },
  setVisible(e) {
    this.setData({ visible_all: Number(e.currentTarget.dataset.v) === 1 })
  },
  setCost(e) {
    const cost_type = e.currentTarget.dataset.v
    this.setData({ cost_type, costLabel: costLabel(cost_type) })
  },
  onName(e) { this.setData({ bill_name: e.detail.value }) },
  onAmount(e) { this.setData({ amount: e.detail.value }) },
  onRemark(e) { this.setData({ remark: e.detail.value }) },
  onDate(e) { this.setData({ consume_date: e.detail.value }) },
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
    if (!p) return
    const cost_type = ['sight', 'hotel', 'food', 'gas', 'transport'].includes(p.point_type)
      ? p.point_type
      : this.data.cost_type
    this.setData({
      day_plan_id: id,
      planName: p.label,
      cost_type,
      costLabel: costLabel(cost_type),
    })
  },
  toggleShare(e) {
    const id = e.currentTarget.dataset.id
    const members = this.data.members.map((m) => (m.user_id === id ? { ...m, checked: !m.checked } : m))
    if (!members.some((m) => m.checked)) {
      wx.showToast({ title: '至少选一人分摊', icon: 'none' })
      return
    }
    this.setData({
      members,
      allShared: members.every((m) => m.checked),
    })
  },
  toggleAllShare() {
    const all = !this.data.allShared
    const userId = this.data.pay_user_id
    const members = this.data.members.map((m) => ({
      ...m,
      checked: all ? true : m.user_id === userId,
    }))
    this.setData({ members, allShared: all })
  },
  async submit() {
    const d = this.data
    if (!d.bill_name || !d.amount) {
      wx.showToast({ title: '请填写名称和金额', icon: 'none' })
      return
    }
    let share_user_ids = d.members.filter((m) => m.checked).map((m) => m.user_id)
    if (!share_user_ids.length) share_user_ids = [d.pay_user_id]
    wx.showLoading({ title: '保存中' })
    try {
      await api.billSave({
        id: d.id,
        travel_id: d.travel_id,
        day_plan_id: d.day_plan_id || null,
        bill_name: d.bill_name,
        amount: Number(d.amount),
        bill_type: 1,
        cost_type: d.cost_type,
        pay_user_id: d.pay_user_id,
        consume_time: `${d.consume_date} 12:00:00`,
        visible_all: d.visible_all,
        share_user_ids,
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
