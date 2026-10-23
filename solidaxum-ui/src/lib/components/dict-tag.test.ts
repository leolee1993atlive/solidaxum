import { describe, expect, it } from 'vitest'
import {
  DICT_TAG_CLASS,
  resolveDictTagClass,
  resolveDictTagInlineStyle,
} from './dict-tag'

describe('resolveDictTagClass', () => {
  it('覆盖 RuoYi listClass 的全部 6 个取值', () => {
    // 值域来自 RuoYi-Vue-Plus/script/sql/ry_vue.sql 的 sys_dict_data.list_class 实测
    for (const k of ['primary', 'success', 'info', 'warning', 'danger', 'default']) {
      expect(Object.keys(DICT_TAG_CLASS)).toContain(k)
    }
  })

  it('antd 透传色名 7 键命中(对齐旧版 renderDict 透传语义)', () => {
    for (const k of ['blue', 'cyan', 'green', 'orange', 'purple', 'red', 'pink']) {
      expect(resolveDictTagClass(k)).not.toBe('')
    }
  })

  it('已知 listClass 返回非空 class', () => {
    expect(resolveDictTagClass('danger')).toBe(DICT_TAG_CLASS.danger)
    expect(resolveDictTagClass('primary')).toBe(DICT_TAG_CLASS.primary)
  })

  it('颜色互不相同(有意别名仅 3 对:primary/blue、success/green、danger/red)', () => {
    const values = Object.values(DICT_TAG_CLASS)
    // 13 键中 3 对刻意同观感(旧前端预设表外色名落 antd 同名色;
    // primary≈蓝/success≈绿/danger≈红 与透传键天然同串),其余互不相同。
    expect(new Set(values).size).toBe(values.length - 3)
  })

  it('空串 / undefined / null 返回空串', () => {
    expect(resolveDictTagClass('')).toBe('')
    expect(resolveDictTagClass(undefined)).toBe('')
    expect(resolveDictTagClass(null)).toBe('')
  })

  it('未知 listClass 返回空串,不抛错', () => {
    expect(resolveDictTagClass('el-check-tag')).toBe('')
    expect(resolveDictTagClass('zzz')).toBe('')
  })
})

describe('resolveDictTagInlineStyle(hex listClass)', () => {
  it('#fa8c16 → antd Tag 观感内联样式(10% 底/实色字/35% 描边)', () => {
    expect(resolveDictTagInlineStyle('#fa8c16')).toBe(
      'background-color:#fa8c161a;color:#fa8c16;border-color:#fa8c1659'
    )
  })

  it('大写 hex 归一为小写', () => {
    expect(resolveDictTagInlineStyle('#0BAAA7')).toBe(
      'background-color:#0baaa71a;color:#0baaa7;border-color:#0baaa759'
    )
  })

  it('语义键/空值返回 null,不占用内联分支', () => {
    expect(resolveDictTagInlineStyle('danger')).toBeNull()
    expect(resolveDictTagInlineStyle('blue')).toBeNull()
    expect(resolveDictTagInlineStyle('')).toBeNull()
    expect(resolveDictTagInlineStyle(undefined)).toBeNull()
  })

  it('非 6 位 hex(3 位/8 位/缺位)一律 null', () => {
    expect(resolveDictTagInlineStyle('#fa8')).toBeNull()
    expect(resolveDictTagInlineStyle('#fa8c16ff')).toBeNull()
    expect(resolveDictTagInlineStyle('#fa8c1')).toBeNull()
  })

  it('批量 hex 值逐一验证内联样式串(10 个样本)', () => {
    const statuses = [
      '#93999b', '#fa8c16', '#a35bfa', '#40a9ff', '#52c41a',
      '#ff5900', '#f5222d', '#faad14', '#0baaa7', '#fa3d1c',
    ]
    for (const hex of statuses) {
      expect(resolveDictTagInlineStyle(hex)).toMatch(
        new RegExp(`^background-color:${hex}1a;color:${hex};border-color:${hex}59$`)
      )
    }
  })
})
