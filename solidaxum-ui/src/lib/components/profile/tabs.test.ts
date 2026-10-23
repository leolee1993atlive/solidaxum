import { describe, expect, it } from 'vitest'
import { PROFILE_TABS, resolveProfileTab } from './tabs.js'

describe('resolveProfileTab', () => {
  it('四个合法值原样返回', () => {
    for (const tab of PROFILE_TABS) {
      expect(resolveProfileTab(tab)).toBe(tab)
    }
  })

  it('缺省(null)回落 userinfo', () => {
    expect(resolveProfileTab(null)).toBe('userinfo')
  })

  it('非法值回落 userinfo,不把脏值透给 Tabs', () => {
    expect(resolveProfileTab('nope')).toBe('userinfo')
    expect(resolveProfileTab('')).toBe('userinfo')
    expect(resolveProfileTab('__proto__')).toBe('userinfo')
  })
})
