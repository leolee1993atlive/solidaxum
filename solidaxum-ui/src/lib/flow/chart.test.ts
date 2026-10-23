import { describe, expect, it } from 'vitest'
import {
  DEFAULT_STATUS_COLORS,
  statusClass,
  statusColorStyle,
} from './chart.js'

describe('statusClass', () => {
  it('2 已完成 / 1 进行中 / 0 未到;字符串数字也认', () => {
    expect(statusClass(2)).toBe('fd-status-done')
    expect(statusClass(1)).toBe('fd-status-todo')
    expect(statusClass(0)).toBe('fd-status-notdone')
    expect(statusClass('2')).toBe('fd-status-done')
  })

  it('定义态(没有 status)不着色', () => {
    expect(statusClass(undefined)).toBe('')
    expect(statusClass(null)).toBe('')
    expect(statusClass(7)).toBe('')
  })
})

describe('statusColorStyle', () => {
  it('把三原色写成 CSS 变量', () => {
    expect(statusColorStyle(['1,2,3', '4,5,6', '7,8,9'])).toBe(
      '--fd-done:1,2,3;--fd-todo:4,5,6;--fd-notdone:7,8,9'
    )
  })

  it('缺省与坏值回落到后端默认色', () => {
    expect(statusColorStyle(null)).toBe(
      `--fd-done:${DEFAULT_STATUS_COLORS[0]};--fd-todo:${DEFAULT_STATUS_COLORS[1]};--fd-notdone:${DEFAULT_STATUS_COLORS[2]}`
    )
    expect(statusColorStyle(['#fff', '4, 5, 6'])).toBe(
      `--fd-done:${DEFAULT_STATUS_COLORS[0]};--fd-todo:4,5,6;--fd-notdone:${DEFAULT_STATUS_COLORS[2]}`
    )
  })
})
