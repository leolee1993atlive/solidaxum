import { apiClient } from '$lib/api/client.js'
import type { DefJson } from '$lib/flow/types.js'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { designerApi } from './designer.js'

// apiClient 是 class 实例;vi.mock 替换整个模块,只验调用形状,不关心返回值。
// vitest 会把 vi.mock 提升到文件顶部,写在 import 之后也生效(prettier 的
// import 排序插件会把 import 归拢到最前面)。
vi.mock('$lib/api/client.js', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
    getList: vi.fn(),
  },
}))

beforeEach(() => vi.clearAllMocks())

const def: DefJson = {
  id: '1761300000000000001',
  flowCode: 'leave',
  flowName: '请假',
  modelValue: 'CLASSICS',
  category: null,
  version: '1',
  isPublish: 0,
  formCustom: null,
  formPath: null,
  listenerType: null,
  listenerPath: null,
  ext: null,
  nodeList: [],
}

describe('saveJson 请求契约', () => {
  // 后端 save_json 读请求头 onlyNodeSkip(warm_flow_ui_handler.rs:46-50);
  // RuoYi 集成下设计器只改节点/跳转,定义级字段由列表页维护,所以默认 true。
  it('POST /warm-flow/save-json,默认带 onlyNodeSkip: "true"', () => {
    designerApi.saveJson(def)
    expect(apiClient.post).toHaveBeenCalledWith('/warm-flow/save-json', def, {
      headers: { onlyNodeSkip: 'true' },
    })
  })

  it('显式 onlyNodeSkip=false 时头为 "false"', () => {
    designerApi.saveJson(def, false)
    expect(apiClient.post).toHaveBeenCalledWith('/warm-flow/save-json', def, {
      headers: { onlyNodeSkip: 'false' },
    })
  })
})

describe('queryDef 请求契约', () => {
  it('有 id 走路径段', () => {
    designerApi.queryDef('1761300000000000001')
    expect(apiClient.get).toHaveBeenCalledWith(
      '/warm-flow/query-def/1761300000000000001'
    )
  })

  // 后端 query_def 的 id 可选,缺省/0 返回空模板(warm_flow_ui_handler.rs:71-76)。
  it('无 id、空串、"0" 都走无参端点(空模板)', () => {
    designerApi.queryDef()
    designerApi.queryDef('')
    designerApi.queryDef('0')
    const urls = vi.mocked(apiClient.get).mock.calls.map((c) => c[0])
    expect(urls).toEqual([
      '/warm-flow/query-def',
      '/warm-flow/query-def',
      '/warm-flow/query-def',
    ])
  })
})

describe('queryFlowChart 请求契约', () => {
  it('实例 id 走路径段', () => {
    designerApi.queryFlowChart('1761300000000000009')
    expect(apiClient.get).toHaveBeenCalledWith(
      '/warm-flow/query-flow-chart/1761300000000000009'
    )
  })
})

describe('handlerResult 请求契约', () => {
  // 后端 HandlerQuery 只实现 handlerType/pageNum/pageSize/status
  // (warm_flow_ui_handler.rs:150-160);Vue 参考 selectUser.vue 还会发
  // handlerCode/handlerName/groupId,被静默忽略,类型不收。
  it('分页数字转字符串,未给的可选项不进查询串', () => {
    designerApi.handlerResult({ handlerType: '用户', pageNum: 1, pageSize: 10 })
    expect(apiClient.get).toHaveBeenCalledWith('/warm-flow/handler-result', {
      handlerType: '用户',
      pageNum: '1',
      pageSize: '10',
    })
  })

  it('可选项 status 有值时带上', () => {
    designerApi.handlerResult({
      handlerType: '角色',
      pageNum: 2,
      pageSize: 20,
      status: '0',
    })
    expect(apiClient.get).toHaveBeenCalledWith('/warm-flow/handler-result', {
      handlerType: '角色',
      pageNum: '2',
      pageSize: '20',
      status: '0',
    })
  })
})

describe('handlerFeedback 请求契约', () => {
  // 后端 parse_storage_ids 兼容逗号拼接与重复键两种形态;这里固定用逗号拼接单值。
  it('storageIds 逗号拼接为单个查询参数', () => {
    designerApi.handlerFeedback(['user:1', 'role:2'])
    expect(apiClient.get).toHaveBeenCalledWith('/warm-flow/handler-feedback', {
      storageIds: 'user:1,role:2',
    })
  })
})

describe('无参 GET 端点路径', () => {
  // 显式元组泛型:混合数组字面量会被推成联合类型,call() 就不可调用了。
  it.each<[string, () => unknown, string]>([
    [
      'listenerList',
      () => designerApi.listenerList(),
      '/warm-flow/listener-list',
    ],
    ['handlerType', () => designerApi.handlerType(), '/warm-flow/handler-type'],
    ['handlerDict', () => designerApi.handlerDict(), '/warm-flow/handler-dict'],
    ['nodeExt', () => designerApi.nodeExt(), '/warm-flow/node-ext'],
    [
      'publishedForm',
      () => designerApi.publishedForm(),
      '/warm-flow/published-form',
    ],
  ])('%s 的请求路径', (_name, call, url) => {
    call()
    expect(apiClient.get).toHaveBeenCalledWith(url)
  })
})
