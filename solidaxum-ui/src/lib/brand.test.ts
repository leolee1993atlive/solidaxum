import { describe, it, expect } from 'vitest'
import { APP_NAME, TOKEN_KEY, CLIENT_ID_KEY, getPageTitle } from './brand.js'

describe('brand', () => {
  it('导出品牌常量', () => {
    // 未注入 VITE_APP_NAME 时兜底 SolidAxum;构建脚本注入后跟随产物名
    expect(APP_NAME).toBe('SolidAxum')
    expect(TOKEN_KEY).toBe('solidaxum_token')
    expect(CLIENT_ID_KEY).toBe('solidaxum_client_id')
  })

  it('getPageTitle 按最长前缀匹配', () => {
    expect(getPageTitle('/dashboard')).toBe('首页')
    expect(getPageTitle('/dashboard/detail')).toBe('首页')
    expect(getPageTitle('/nowhere')).toBeUndefined()
  })
})
