import type { RouterVo } from '$lib/api/types.js'
import { describe, it, expect } from 'vitest'
import { routersToCacheMap } from './cache-map.js'

const tree: RouterVo[] = [
  {
    path: '/system',
    meta: { title: '系统管理' },
    children: [
      {
        path: 'user',
        component: 'system/user/index',
        meta: { title: '用户管理' },
      }, // 缺 noCache → 缓存
      {
        path: 'config',
        component: 'system/config/index',
        meta: { title: '参数', noCache: true },
      }, // 不缓存
      {
        path: 'log',
        meta: { title: '日志管理' },
        children: [
          {
            path: 'operlog',
            component: 'monitor/operlog/index',
            meta: { title: '操作日志' },
          },
        ],
      },
    ],
  },
]

describe('routersToCacheMap', () => {
  it('marks leaf cacheable when noCache !== true', () => {
    const m = routersToCacheMap(tree)
    expect(m['/system/user']).toBe(true)
    expect(m['/system/config']).toBe(false)
  })
  it('handles nested full paths', () => {
    const m = routersToCacheMap(tree)
    expect(m['/system/log/operlog']).toBe(true)
  })
  it('does not add directory (non-leaf) paths', () => {
    const m = routersToCacheMap(tree)
    expect('/system/log' in m).toBe(false)
  })
})
