import { describe, it, expect, beforeEach } from 'vitest'
import { setHiddenRouteMap, hiddenRouteFor } from './hidden-routes.js'

beforeEach(() => setHiddenRouteMap({}))

describe('hiddenRouteFor', () => {
  it('按精确路径命中(oss-config 场景)', () => {
    setHiddenRouteMap({
      '/system/oss-config': {
        title: '文件配置管理',
        activeMenu: '/system/oss',
      },
    })
    expect(hiddenRouteFor('/system/oss-config')).toEqual({
      title: '文件配置管理',
      activeMenu: '/system/oss',
    })
  })

  it('匹配 [param] 必填动态段(role-auth/user 场景)', () => {
    setHiddenRouteMap({
      '/system/role-auth/user/[roleId]': {
        title: '分配用户',
        activeMenu: '/system/role',
      },
    })
    expect(hiddenRouteFor('/system/role-auth/user/5')).toEqual({
      title: '分配用户',
      activeMenu: '/system/role',
    })
    // 缺动态段不命中
    expect(hiddenRouteFor('/system/role-auth/user')).toBeUndefined()
  })

  it('匹配 [[param]] 可选动态段:带不带值都命中(leaveEdit 场景)', () => {
    setHiddenRouteMap({
      '/workflow/leaveEdit/[[id]]': { title: '请假申请' },
    })
    expect(hiddenRouteFor('/workflow/leaveEdit')?.title).toBe('请假申请')
    expect(hiddenRouteFor('/workflow/leaveEdit/123')?.title).toBe('请假申请')
  })

  it('未知路径返回 undefined', () => {
    setHiddenRouteMap({
      '/system/oss-config': { title: '文件配置管理' },
    })
    expect(hiddenRouteFor('/system/user')).toBeUndefined()
  })

  it('不过度匹配前缀兄弟路径(/system/oss 不吃掉 /system/oss-config)', () => {
    setHiddenRouteMap({ '/system/oss': { title: 'OSS管理' } })
    expect(hiddenRouteFor('/system/oss-config')).toBeUndefined()
    expect(hiddenRouteFor('/system/oss')).toEqual({ title: 'OSS管理' })
  })

  it('activeMenu 缺省时为 undefined', () => {
    setHiddenRouteMap({ '/workflow/design/index': { title: '流程设计' } })
    expect(hiddenRouteFor('/workflow/design/index')?.activeMenu).toBeUndefined()
  })
})
