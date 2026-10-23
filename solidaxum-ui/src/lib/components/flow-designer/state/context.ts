import { getContext, setContext } from 'svelte'
import type { DesignerStore } from './designer.svelte.js'

// 自定义边 / 节点由 xyflow 在 <SvelteFlow> 子树里实例化,拿不到 Canvas 的 prop,
// 但 Svelte context 沿组件树可达:Designer 设一次,画布里谁要 store 谁取。
// FlowChart(实例流程图)不设 → 取到 undefined → 编辑钩子一律不挂。
const KEY = Symbol('flow-designer')

export function setDesignerContext(store: DesignerStore) {
  setContext(KEY, store)
}

export function getDesignerContext(): DesignerStore | undefined {
  return getContext<DesignerStore | undefined>(KEY)
}
