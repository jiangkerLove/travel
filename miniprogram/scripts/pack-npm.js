const fs = require('fs')
const path = require('path')

const root = path.join(__dirname, '..')
const pkg = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8'))
const deps = Object.keys(pkg.dependencies || {})
const outRoot = path.join(root, 'miniprogram_npm')

function copyDir(src, dest) {
  fs.mkdirSync(dest, { recursive: true })
  for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
    const from = path.join(src, entry.name)
    const to = path.join(dest, entry.name)
    if (entry.isDirectory()) copyDir(from, to)
    else fs.copyFileSync(from, to)
  }
}

fs.rmSync(outRoot, { recursive: true, force: true })
fs.mkdirSync(outRoot, { recursive: true })

for (const name of deps) {
  const pkgDir = path.join(root, 'node_modules', name)
  const metaPath = path.join(pkgDir, 'package.json')
  if (!fs.existsSync(metaPath)) {
    throw new Error(`缺少依赖 ${name}，请先在 miniprogram 目录执行 npm install`)
  }
  const meta = JSON.parse(fs.readFileSync(metaPath, 'utf8'))
  const distRel = meta.miniprogram || 'miniprogram_dist'
  const distDir = path.join(pkgDir, distRel)
  if (!fs.existsSync(distDir)) {
    throw new Error(`${name} 没有小程序构建目录 ${distRel}`)
  }
  const dest = path.join(outRoot, name)
  copyDir(distDir, dest)
  fs.copyFileSync(metaPath, path.join(dest, 'package.json'))
  console.log(`packed ${name} -> miniprogram_npm/${name}`)
}
