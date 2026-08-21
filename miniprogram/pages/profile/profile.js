function decorateUser(user) {
  const u = user || {}
  const name = u.nickname || '旅行者'
  const life = u.work_life || null
  const bits = []
  if (life && life.ready) {
    if (life.age) bits.push(`${life.age}岁`)
    if (life.genderText) bits.push(life.genderText)
  }
  return {
    ...u,
    nickname: name,
    initial: name.slice(0, 1),
    life,
    sub: bits.join(' · '),
  }
}

Page({
  data: {
    user: {},
    initial: '旅',
    sub: '',
    life: null,
  },
  async onShow() {
    const ok = await getApp().ensureLogin()
    if (!ok) {
      wx.reLaunch({ url: '/pages/boot/boot' })
      return
    }
    let user = getApp().globalData.user || wx.getStorageSync('user') || {}
    try {
      user = await getApp().refreshUser()
    } catch (e) {}
    this.applyUser(user)
  },
  applyUser(user) {
    const u = decorateUser(user)
    this.setData({
      user: u,
      initial: u.initial,
      sub: u.sub,
      life: u.life,
    })
  },
  goEdit() { wx.navigateTo({ url: '/pages/profile/edit' }) },
  goHistory() { wx.navigateTo({ url: '/pages/mine/history' }) },
  goAbout() { wx.navigateTo({ url: '/pages/mine/about' }) },
})
