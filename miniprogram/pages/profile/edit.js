const { api } = require('../../utils/api')

function decorateUser(user) {
  const u = user || {}
  const name = (u.nickname || '旅行者').trim() || '旅行者'
  return {
    ...u,
    nickname: name,
    initial: name.slice(0, 1),
  }
}

Page({
  data: {
    avatar: '',
    nickname: '',
    initial: '旅',
    dirty: false,
    saving: false,
  },
  onLoad() {
    this.syncFromStore()
  },
  syncFromStore() {
    const u = decorateUser(getApp().globalData.user || wx.getStorageSync('user') || {})
    this._originAvatar = u.avatar || ''
    this._originNick = u.nickname
    this.setData({
      avatar: this._originAvatar,
      nickname: this._originNick,
      initial: u.initial,
      dirty: false,
    })
  },
  markDirty() {
    const dirty =
      (this.data.avatar || '') !== (this._originAvatar || '') ||
      (this.data.nickname || '').trim() !== (this._originNick || '')
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
    const payload = {}
    if (nickname !== this._originNick) payload.nickname = nickname
    if ((this.data.avatar || '') !== (this._originAvatar || '')) {
      payload.avatar = this.data.avatar
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
      const u = decorateUser(user)
      this._originAvatar = u.avatar || ''
      this._originNick = u.nickname
      this.setData({
        avatar: this._originAvatar,
        nickname: this._originNick,
        initial: u.initial,
        dirty: false,
      })
      wx.showToast({ title: '已保存', icon: 'success' })
      setTimeout(() => wx.navigateBack(), 400)
    } finally {
      wx.hideLoading()
      this.setData({ saving: false })
    }
  },
})
