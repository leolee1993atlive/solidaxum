import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

const menuSrc = readFileSync(
  join(process.cwd(), 'src', 'routes', '(app)', 'system', 'menu', '+page.svelte'),
  'utf8'
)
const iconSelectSrc = readFileSync(
  join(process.cwd(), 'src', 'lib', 'components', 'IconSelect.svelte'),
  'utf8'
)

describe('menu 页护栏', () => {
  // 选择器的选项必须来自 icon-map,不能直接列 lucide 全集 ——
  // 表里没有的名字存进去,侧边栏渲染成默认方块。
  it('IconSelect 的选项来自 icon-map,不是硬编码', () => {
    expect(iconSelectSrc).toContain('filterIconNames')
    expect(iconSelectSrc).toContain('resolveMenuIcon')
  })

  it('菜单弹窗用 IconSelect,不再是裸文本框', () => {
    expect(menuSrc).toContain('IconSelect')
  })

  // 🔴 本仓 isFrame 值域是 0=是外链/1=非外链,与 Vue 的 'Y' 方向相反。
  // 写成 'Y' 会把库里 141 个普通菜单全标成「外链」。
  it("外链判定用 isFrame === '0',不是 Vue 的 'Y'", () => {
    expect(menuSrc).toMatch(/isFrame === '0'/)
    expect(menuSrc).not.toMatch(/isFrame === 'Y'/)
  })

  // 九个字段的联动显隐,少一个就会出现「按钮类型仍显示组件路径」。
  it.each([
    'showForNonButton',
    'showForMenuOnly',
    'showPerms',
    'showActiveMenu',
  ])('联动显隐条件 %s 存在', (name) => {
    expect(menuSrc).toContain(name)
  })

  // 两个字段曾整块缺失。
  it.each(['queryParam', 'activeMenu'])('弹窗引用 %s', (field) => {
    expect(menuSrc).toContain(field)
  })
})
