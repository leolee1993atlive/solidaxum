import type { TreeSelectNode } from '$lib/components/tree-select-types.js'
import type { SegmentedOption } from '$lib/components/crud/index.js'

/** 下拉选项。 */
export type QueryOption = { label: string; value: string }

/**
 * QueryForm 字段描述。
 * `type` 可省略,省略即 'text' —— 保证 23 个现存调用方的 {key,label,placeholder} 写法不用改。
 */
export type QueryField =
  | { key: string; label: string; type?: 'text'; placeholder?: string }
  | {
      key: string
      label: string
      type: 'select'
      /** 静态选项;与 dictType 二选一。 */
      options?: QueryOption[]
      /** 字典码,组件自行 fetchDict 拉取。 */
      dictType?: string
      placeholder?: string
    }
  | {
      key: string
      label: string
      type: 'multiSelect'
      /** 静态选项;与 dictType 二选一。 */
      options?: QueryOption[]
      /** 字典码,组件自行 fetchDict 拉取。 */
      dictType?: string
      placeholder?: string
    }
  | {
      key: string
      label: string
      type: 'segmented'
      /** 分段选项;需要「全部」时由页面自带 {label:'全部', value:''} 项。 */
      options: SegmentedOption[]
    }
  | {
      key: string
      label: string
      type: 'dateRange'
      /** 起始日期写入 model 的键名,默认 'beginTime'。 */
      beginKey?: string
      /** 结束日期写入 model 的键名,默认 'endTime'。 */
      endKey?: string
    }
  | {
      key: string
      label: string
      type: 'treeSelect'
      treeData: TreeSelectNode[]
      placeholder?: string
    }
  | {
      key: string
      label: string
      type: 'userSelect'
      /** 未传时落回 UserPicker 默认「请选择人员」。 */
      placeholder?: string
    }
