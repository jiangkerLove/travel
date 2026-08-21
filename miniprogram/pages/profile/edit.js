const { api } = require('../../utils/api')

const GENDER_RANGE = ['男', '女']
const ROLE_RANGE = ['女职工', '女干部']

function todayStr() {
  const n = new Date()
  const m = `${n.getMonth() + 1}`.padStart(2, '0')
  const d = `${n.getDate()}`.padStart(2, '0')
  return `${n.getFullYear()}-${m}-${d}`
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
  const [y, m, d] = birthday.split('-')
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
    birthdayText: '',
    gender: 0,
    genderIndex: 0,
    genderLabel: '',
    genderRange: GENDER_RANGE,
    femaleRole: 0,
    roleLabel: ROLE_RANGE[0],
    roleRange: ROLE_RANGE,
    workStartYear: 0,
    workYearValue: todayStr(),
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
    this._origin = {
      avatar: u.avatar || '',
      nickname: u.nickname,
      birthday: u.birthday || '',
      gender,
      femaleRole,
      workStartYear,
    }
    this.setData({
      avatar: this._origin.avatar,
      nickname: this._origin.nickname,
      initial: u.initial,
      birthday: this._origin.birthday,
      birthdayText: birthdayText(this._origin.birthday),
      gender,
      genderIndex: gender === 2 ? 1 : 0,
      genderLabel: gender === 1 ? '男' : gender === 2 ? '女' : '',
      femaleRole,
      roleLabel: ROLE_RANGE[femaleRole] || ROLE_RANGE[0],
      workStartYear,
      workYearValue: workStartYear ? `${workStartYear}` : `${new Date().getFullYear()}`,
      dirty: false,
    })
  },
  markDirty() {
    const o = this._origin
    const dirty =
      (this.data.avatar || '') !== (o.avatar || '') ||
      (this.data.nickname || '').trim() !== (o.nickname || '') ||
      (this.data.birthday || '') !== (o.birthday || '') ||
      Number(this.data.gender) !== Number(o.gender) ||
      Number(this.data.femaleRole) !== Number(o.femaleRole) ||
      Number(this.data.workStartYear) !== Number(o.workStartYear)
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
    const birthday = e.detail.value || ''
    this.setData({ birthday, birthdayText: birthdayText(birthday) }, () => this.markDirty())
  },
  onGender(e) {
    const genderIndex = Number(e.detail.value)
    const gender = genderIndex === 1 ? 2 : 1
    this.setData({
      genderIndex,
      gender,
      genderLabel: GENDER_RANGE[genderIndex],
    }, () => this.markDirty())
  },
  onRole(e) {
    const femaleRole = Number(e.detail.value) || 0
    this.setData({
      femaleRole,
      roleLabel: ROLE_RANGE[femaleRole],
    }, () => this.markDirty())
  },
  onWorkYear(e) {
    const raw = e.detail.value || ''
    const workStartYear = Number(String(raw).slice(0, 4)) || 0
    this.setData({
      workStartYear,
      workYearValue: workStartYear ? `${workStartYear}` : `${new Date().getFullYear()}`,
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
    const o = this._origin
    const payload = {}
    if (nickname !== o.nickname) payload.nickname = nickname
    if ((this.data.avatar || '') !== (o.avatar || '')) payload.avatar = this.data.avatar
    if ((this.data.birthday || '') !== (o.birthday || '')) payload.birthday = this.data.birthday
    if (Number(this.data.gender) !== Number(o.gender)) payload.gender = Number(this.data.gender)
    if (Number(this.data.femaleRole) !== Number(o.femaleRole)) payload.female_role = Number(this.data.femaleRole)
    if (Number(this.data.workStartYear) !== Number(o.workStartYear) && this.data.workStartYear) {
      payload.work_start_year = Number(this.data.workStartYear)
    }
    if (!Object.keys(payload).length) {
      this.setData({ dirty: false })
      return
    }
    this.setData({ saving: true })
    wx.showLoading({ title: '保存中' })
    try {
      const user = await api.updateUser(payload)
      getApp().setUser(user)
      this.syncFromStore()
      wx.showToast({ title: '已保存', icon: 'success' })
      setTimeout(() => wx.navigateBack(), 400)
    } finally {
      wx.hideLoading()
      this.setData({ saving: false })
    }
  },
})
