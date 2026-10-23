import { describe, it, expect, afterEach } from 'vitest'
import {
  routersToNavTree,
  routersToHiddenMap,
  pathToTitle,
  flattenNavLinks,
} from './transform.js'
import { setHiddenRouteMap } from '$lib/stores/hidden-routes.js'
import type { RouterVo } from '$lib/api/types.js'

/** 造一棵 mock 菜单树(对齐后端 getRouters 返回的 RouterVo)。 */
function mockTree(): RouterVo[] {
  return [
    {
      // 顶层目录:component=Layout,作为 NavNode
      name: 'System',
      path: '/system',
      component: 'Layout',
      meta: { title: '系统管理', icon: 'system' },
      children: [
        {
          name: 'User',
          path: 'user',
          component: 'system/user/index',
          meta: { title: '用户管理', icon: 'user' },
        },
        {
          name: 'Role',
          path: 'role',
          component: 'system/role/index',
          meta: { title: '角色管理', icon: 'peoples' },
        },
        {
          // hidden 的子节点应被过滤
          name: 'Hidden',
          path: 'hidden',
          hidden: true,
          component: 'system/hidden/index',
          meta: { title: '隐藏页' },
        },
        {
          name: 'Log',
          path: 'log',
          meta: { title: '日志管理', icon: 'log' },
          children: [
            {
              name: 'Operlog',
              path: 'operlog',
              component: 'monitor/operlog/index',
              meta: { title: '操作日志', icon: 'form' },
            },
            {
              name: 'Logininfo',
              path: 'logininfo',
              component: 'monitor/logininfo/index',
              meta: { title: '登录日志', icon: 'logininfo' },
            },
          ],
        },
      ],
    },
    {
      // 另一个顶层目录,带二级目录
      name: 'Monitor',
      path: '/monitor',
      component: 'Layout',
      meta: { title: '系统监控', icon: 'monitor' },
      children: [
        {
          name: 'Online',
          path: 'online',
          component: 'monitor/online/index',
          meta: { title: '在线用户', icon: 'online' },
        },
      ],
    },
    {
      // 外链菜单
      name: 'Doc',
      path: 'doc',
      component: 'Layout',
      meta: { title: '文档', icon: 'guide', link: 'https://example.com' },
    },
    {
      // 顶层 hidden 的整个目录应被过滤
      name: 'HiddenGroup',
      path: '/hidden-group',
      component: 'Layout',
      hidden: true,
      meta: { title: '隐藏组' },
      children: [
        { name: 'X', path: 'x', component: 'x/index', meta: { title: 'X' } },
      ],
    },
  ]
}

describe('routersToNavTree', () => {
  it('keeps top-level directories as collapsible nodes with items', () => {
    const tree = routersToNavTree(mockTree())
    const sys = tree.find((n) => n.title === '系统管理')!
    expect(sys.url).toBeUndefined()
    expect(sys.items?.map((i) => i.title)).toContain('用户管理')
  })

  it('joins full path across arbitrary depth (3 levels)', () => {
    const tree = routersToNavTree(mockTree())
    const sys = tree.find((n) => n.title === '系统管理')!
    const log = sys.items?.find((i) => i.title === '日志管理')!
    const operlog = log.items?.find((i) => i.title === '操作日志')!
    expect(operlog.url).toBe('/system/log/operlog')
  })

  it('leaf children become url nodes without items', () => {
    const tree = routersToNavTree(mockTree())
    const sys = tree.find((n) => n.title === '系统管理')!
    const user = sys.items?.find((i) => i.title === '用户管理')!
    expect(user.url).toBe('/system/user')
    expect(user.items).toBeUndefined()
  })

  it('filters hidden top-level and hidden children', () => {
    const tree = routersToNavTree(mockTree())
    expect(tree.map((n) => n.title)).not.toContain('隐藏组')
    const sys = tree.find((n) => n.title === '系统管理')!
    expect(sys.items?.find((i) => i.title === '隐藏页')).toBeUndefined()
  })

  it('external-link top-level becomes a url node', () => {
    const tree = routersToNavTree(mockTree())
    const doc = tree.find((n) => n.title === '文档')!
    expect(doc.url).toBe('https://example.com')
  })

  it('returns empty array for empty input', () => {
    expect(routersToNavTree([])).toEqual([])
  })
})

describe('routersToHiddenMap', () => {
  it('收集 hidden 叶子的标题与 activeMenu(oss-config 场景)', () => {
    const routers: RouterVo[] = [
      {
        name: 'System',
        path: '/system',
        component: 'Layout',
        meta: { title: '系统管理' },
        children: [
          {
            name: 'Oss',
            path: 'oss',
            component: 'system/oss/index',
            meta: { title: 'OSS管理' },
          },
          {
            name: 'OssConfig',
            path: 'oss-config',
            hidden: true,
            component: 'system/oss/config',
            meta: { title: '文件配置管理', activeMenu: '/system/oss' },
          },
        ],
      },
    ]
    expect(routersToHiddenMap(routers)).toEqual({
      '/system/oss-config': {
        title: '文件配置管理',
        activeMenu: '/system/oss',
      },
    })
  })

  it('保留动态段原样;activeMenu 缺省时不产出该键', () => {
    const routers: RouterVo[] = [
      {
        name: 'Workflow',
        path: '/workflow',
        component: 'Layout',
        meta: { title: '工作流程' },
        children: [
          {
            name: 'LeaveEdit',
            path: 'leaveEdit/[[id]]',
            hidden: true,
            component: 'workflow/leave/leaveEdit',
            meta: { title: '请假申请' },
          },
        ],
      },
    ]
    expect(routersToHiddenMap(routers)).toEqual({
      '/workflow/leaveEdit/[[id]]': { title: '请假申请' },
    })
  })

  it('可见叶子不入表', () => {
    // mockTree 里只有一个 hidden 叶子:/system/hidden
    expect(routersToHiddenMap(mockTree())).toEqual({
      '/system/hidden': { title: '隐藏页' },
    })
  })

  it('空输入返回空表', () => {
    expect(routersToHiddenMap([])).toEqual({})
  })
})

describe('pathToTitle', () => {
  afterEach(() => setHiddenRouteMap({}))

  it('looks up title by full path across the tree', () => {
    const tree = routersToNavTree(mockTree())
    expect(pathToTitle(tree, '/system/user')).toBe('用户管理')
    expect(pathToTitle(tree, '/system/log/operlog')).toBe('操作日志')
    expect(pathToTitle(tree, '/system/log/logininfo')).toBe('登录日志')
  })
  it('returns undefined for unknown path', () => {
    const tree = routersToNavTree(mockTree())
    expect(pathToTitle(tree, '/no/such/path')).toBeUndefined()
  })
  it('falls back to hidden-route title table for non-menu pages', () => {
    const tree = routersToNavTree(mockTree())
    // 设计器(隐藏菜单):无论带不带 /index 后缀
    expect(pathToTitle(tree, '/workflow/design/index')).toBe('流程设计器')
    expect(pathToTitle(tree, '/workflow/design')).toBe('流程设计器')
    // 代码生成编辑配置(隐藏菜单,带动态 tableId 段)
    expect(pathToTitle(tree, '/tool/gen-edit/index/123456789')).toBe(
      '修改生成配置'
    )
  })
  it('getRouters 隐藏菜单数据优先于静态兜底表', () => {
    const tree = routersToNavTree(mockTree())
    setHiddenRouteMap({
      '/system/oss-config': {
        title: '文件配置管理',
        activeMenu: '/system/oss',
      },
      // DB 菜单名为「流程设计」,静态表为「流程设计器」:以 DB 为准(对齐 Vue)
      '/workflow/design/index': { title: '流程设计' },
    })
    expect(pathToTitle(tree, '/system/oss-config')).toBe('文件配置管理')
    expect(pathToTitle(tree, '/workflow/design/index')).toBe('流程设计')
    // 隐藏菜单表未覆盖的仍走静态兜底
    expect(pathToTitle(tree, '/workflow/design')).toBe('流程设计器')
  })
  it('隐藏菜单表支持动态段匹配', () => {
    const tree = routersToNavTree(mockTree())
    setHiddenRouteMap({
      '/system/role-auth/user/[roleId]': {
        title: '分配用户',
        activeMenu: '/system/role',
      },
    })
    expect(pathToTitle(tree, '/system/role-auth/user/9')).toBe('分配用户')
  })
})

describe('flattenNavLinks', () => {
  it('collects all leaf urls recursively', () => {
    const tree = routersToNavTree(mockTree())
    const links = flattenNavLinks(tree)
    const urls = links.map((l) => l.url)
    expect(urls).toContain('/system/user')
    expect(urls).toContain('/system/role')
    expect(urls).toContain('/monitor/online')
    expect(urls).toContain('/system/log/operlog')
    expect(urls).toContain('/system/log/logininfo')
  })

  it('excludes external links', () => {
    const tree = routersToNavTree(mockTree())
    const links = flattenNavLinks(tree)
    expect(links.find((l) => l.url === 'https://example.com')).toBeUndefined()
  })

  it('returns empty array for empty input', () => {
    expect(flattenNavLinks([])).toEqual([])
  })
})
