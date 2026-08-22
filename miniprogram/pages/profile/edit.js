const { api } = require('../../utils/api')

const GENDER_RANGE = ['请选择', '男', '女']
const ROLE_RANGE = ['女职工', '女干部']

function todayStr() {
  const n = new Date()
  const m = `${n.getMonth() + 1}`.padStart(2, '0')
  const d = `${n.getDate()}`.padStart(2, '0')
  return `${n.getFullYear()}-${m}-${d}`
}

function monthStr(d = new Date()) {
  const m = `${d.getMonth() + 1}`.padStart(2, '0')
  return `${d.getFullYear()}-${m}`
}

function workStartText(year, month) {
  if (!year) return ''
  const m = Number(month) || 1
  return `${year}年${m}月`
}

function decorateUser(user) {
  const u = user || {}
  const name = (u.nickname || '旅行者').trim() || '旅行者'
  return {
    ...u,
    nickname: name,
    initial: name.slice(0, 1),
  }
}

function birthdayText(birthday) {
  if (!birthday) return ''
  const [y, m, d] = String(birthday).split('-')
  if (!y || !m || !d) return ''
  const now = new Date()
  let age = now.getFullYear() - Number(y)
  const md = `${now.getMonth() + 1}`.padStart(2, '0') + '-' + `${now.getDate()}`.padStart(2, '0')
  if (`${m}-${d}` > md) age -= 1
  return `${Number(y)}年${Number(m)}月${Number(d)}日 · ${age}岁`
}

Page({
  data: {
    avatar: '',
    nickname: '',
    initial: '旅',
    birthday: '',
    birthdayValue: '1990-01-01',
    birthdayText: '',
    gender: 0,
    genderIndex: 0,
    genderLabel: '',
    genderRange: GENDER_RANGE,
    femaleRole: 0,
    roleLabel: ROLE_RANGE[0],
    roleRange: ROLE_RANGE,
    workStartYear: 0,
    workStartMonth: 0,
    workMonthValue: monthStr(),
    workMonthEnd: monthStr(),
    workStartText: '',
    today: todayStr(),
    dirty: false,
    saving: false,
  },
  onLoad() {
    this.syncFromStore()
  },
  syncFromStore() {
    const u = decorateUser(getApp().globalData.user || wx.getStorageSync('user') || {})
    const gender = Number(u.gender) || 0
    const femaleRole = Number(u.female_role) || 0
    const workStartYear = Number(u.work_start_year) || 0
    const workStartMonth = Number(u.work_start_month) || (workStartYear ? 1 : 0)
    const birthday = u.birthday || ''
    this._origin = {
      avatar: u.avatar || '',
      nickname: u.nickname,
      birthday,
      gender,
      femaleRole,
      workStartYear,
      workStartMonth,
    }
    this.setData({
      avatar: this._origin.avatar,
      nickname: this._origin.nickname,
      initial: u.initial,
      birthday,
      birthdayValue: birthday || '1990-01-01',
      birthdayText: birthdayText(birthday),
      gender,
      genderIndex: gender === 1 ? 1 : gender === 2 ? 2 : 0,
      genderLabel: gender === 1 ? '男' : gender === 2 ? '女' : '',
      femaleRole,
      roleLabel: ROLE_RANGE[femaleRole] || ROLE_RANGE[0],
      workStartYear,
      workStartMonth,
      workMonthValue: workStartYear
        ? `${workStartYear}-${String(workStartMonth || 1).padStart(2, '0')}`
        : monthStr(),
      workMonthEnd: monthStr(),
      workStartText: workStartText(workStartYear, workStartMonth),
      dirty: false,
    })
  },
  snapshot() {
    return {
      avatar: this.data.avatar || '',
      nickname: (this.data.nickname || '').trim(),
      birthday: this.data.birthday || '',
      gender: Number(this.data.gender) || 0,
      femaleRole: Number(this.data.femaleRole) || 0,
      workStartYear: Number(this.data.workStartYear) || 0,
      workStartMonth: Number(this.data.workStartMonth) || 0,
    }
  },
  markDirty() {
    const o = this._origin
    const n = this.snapshot()
    const dirty =
      n.avatar !== (o.avatar || '') ||
      n.nickname !== (o.nickname || '') ||
      n.birthday !== (o.birthday || '') ||
      n.gender !== Number(o.gender) ||
      n.femaleRole !== Number(o.femaleRole) ||
      n.workStartYear !== Number(o.workStartYear) ||
      n.workStartMonth !== Number(o.workStartMonth)
    if (dirty !== this.data.dirty) this.setData({ dirty })
  },
  onChooseAvatar(e) {
    const avatar = e.detail && e.detail.avatarUrl
    if (!avatar) return
    this.setData({ avatar }, () => this.markDirty())
  },
  onNickInput(e) {
    this.setData({ nickname: e.detail.value || '' }, () => this.markDirty())
  },
  onNickChange(e) {
    this.setData({ nickname: e.detail.value || '' }, () => this.markDirty())
  },
  onBirthday(e) {
    const birthday = (e.detail && e.detail.value) || ''
    this.setData({
      birthday,
      birthdayValue: birthday || '1990-01-01',
      birthdayText: birthdayText(birthday),
    }, () => this.markDirty())
  },
  onGender(e) {
    const genderIndex = Number(e.detail && e.detail.value)
    const gender = genderIndex === 1 ? 1 : genderIndex === 2 ? 2 : 0
    this.setData({
      genderIndex,
      gender,
      genderLabel: gender === 1 ? '男' : gender === 2 ? '女' : '',
    }, () => this.markDirty())
  },
  onRole(e) {
    const femaleRole = Number(e.detail && e.detail.value) || 0
    this.setData({
      femaleRole,
      roleLabel: ROLE_RANGE[femaleRole],
    }, () => this.markDirty())
  },
  onWorkMonth(e) {
    const raw = (e.detail && e.detail.value) || ''
    const parts = String(raw).split('-')
    const workStartYear = Number(parts[0]) || 0
    const workStartMonth = Number(parts[1]) || 1
    this.setData({
      workStartYear,
      workStartMonth,
      workMonthValue: workStartYear
        ? `${workStartYear}-${String(workStartMonth).padStart(2, '0')}`
        : monthStr(),
      workStartText: workStartText(workStartYear, workStartMonth),
    }, () => this.markDirty())
  },
  async onSave() {
    if (!this.data.dirty || this.data.saving) return
    const nickname = (this.data.nickname || '').trim()
    if (!nickname) {
      wx.showToast({ title: '请填写昵称', icon: 'none' })
      return
    }
    if (nickname.length > 20) {
      wx.showToast({ title: '昵称过长', icon: 'none' })
      return
    }
    const payload = { nickname }
    if (this.data.avatar) payload.avatar = this.data.avatar
    if (this.data.birthday) payload.birthday = this.data.birthday
    if (this.data.gender === 1 || this.data.gender === 2) payload.gender = this.data.gender
    payload.female_role = Number(this.data.femaleRole) || 0
    if (this.data.workStartYear) {
      payload.work_start_year = Number(this.data.workStartYear)
      payload.work_start_month = Number(this.data.workStartMonth) || 1
    }
    this.setData({ saving: true })
    wx.showLoading({ title: '保存中' })
    try {
      const user = await api.updateUser(payload)
      const prev = getApp().globalData.user || {}
      getApp().setUser({
        ...prev,
        ...(user || {}),
        ...payload,
        work_life: (user && (user.work_life || user.workLife)) || null,
      })
      this.syncFromStore()
      wx.showToast({ title: '已保存', icon: 'success' })
      setTimeout(() => wx.navigateBack(), 400)
    } finally {
      wx.hideLoading()
      this.setData({ saving: false })
    }
  },
})
