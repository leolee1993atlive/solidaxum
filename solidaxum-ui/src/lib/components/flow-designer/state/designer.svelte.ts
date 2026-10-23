import { designerApi } from '$lib/api/workflow/designer.js'
import {
  copySelection,
  pasteClipboard,
  type Clipboard,
} from '$lib/flow/clipboard.js'
import { defJsonToFlow } from '$lib/flow/deserialize.js'
import {
  insertNodeOnEdge,
  renameNodeCode,
  updateEdgeJson,
  updateNodeJson,
} from '$lib/flow/edit.js'
import { flowToDefJson } from '$lib/flow/serialize.js'
import type { Graph } from '$lib/flow/snapshot.js'
import type {
  DefJson,
  DefJsonBase,
  FlowNode,
  NodeJsonFields,
  Point,
  SkipEdge,
  SkipJsonFields,
} from '$lib/flow/types.js'
import { validateDefinition, type ValidationIssue } from '$lib/flow/validate.js'
import { History } from './history.svelte.js'

/**
 * 设计器画布状态。nodes/edges 直接 bind 给 <SvelteFlow>,拖拽、选中、连线都由
 * 它回写到这两个数组,这里不再维护第二份副本。
 */
export class DesignerStore {
  nodes = $state.raw<FlowNode[]>([])
  edges = $state.raw<SkipEdge[]>([])
  base = $state.raw<DefJsonBase | null>(null)
  warnings = $state.raw<string[]>([])
  loading = $state(false)
  saving = $state(false)
  /** 最近一次加载/保存时的定义串;选中态、视口不进 DefJson,天然不算改动 */
  private savedSnapshot = ''
  /** 撤销 / 重做栈;提交点见 commit() 的调用处,保存不清栈 */
  readonly history = new History()
  /** 会话内存剪贴板,不接系统剪贴板 */
  clipboard = $state.raw<Clipboard | null>(null)
  /** 面板拖入时鼠标下的候选跳转 id;只在拖拽期间非空,SkipEdge 据此高亮 */
  dropTargetEdgeId = $state<string | null>(null)
  /** 拖动 / 连线 / 重连进行中:撤销重做暂停,免得半途的拖动把恢复后的状态又写回去 */
  interacting = false

  setDef(def: DefJson) {
    const g = defJsonToFlow(def)
    this.nodes = g.nodes
    this.edges = g.edges
    this.base = g.base
    this.warnings = g.warnings
    this.savedSnapshot = this.snapshot()
    this.history.reset({ nodes: this.nodes, edges: this.edges })
  }

  async load(definitionId: string) {
    this.loading = true
    try {
      const resp = await designerApi.queryDef(definitionId)
      if (!resp.data) throw new Error('流程定义为空')
      this.setDef(resp.data)
    } finally {
      this.loading = false
    }
  }

  /** 保存用的 DefJson,由画布实时状态反算(规格 §8)。 */
  toDefJson(): DefJson {
    if (!this.base) throw new Error('尚未加载流程定义')
    return flowToDefJson({
      nodes: this.nodes,
      edges: this.edges,
      base: this.base,
    })
  }

  private snapshot(): string {
    return this.base ? JSON.stringify(this.toDefJson()) : ''
  }

  /** 与上次加载/保存相比是否有几何或属性改动。按需调用(关闭前确认),不做成响应式 */
  isDirty(): boolean {
    return this.base !== null && this.snapshot() !== this.savedSnapshot
  }

  // ── 属性面板 ──────────────────────────────────────────────────────────

  /** 当前选中的节点/边(多选时取第一个);面板据此路由到对应表单 */
  get selectedNode(): FlowNode | undefined {
    return this.nodes.find((n) => n.selected)
  }

  get selectedEdge(): SkipEdge | undefined {
    return this.edges.find((e) => e.selected)
  }

  // 面板逐字符写回:同一元素同一字段的连续输入按合并键并成一步(history.svelte.ts)
  updateNodeJson(id: string, patch: Partial<NodeJsonFields>) {
    this.nodes = updateNodeJson(this.nodes, id, patch)
    this.commit(`node:${id}:${Object.keys(patch).sort().join(',')}`)
  }

  updateEdgeJson(id: string, patch: Partial<SkipJsonFields>) {
    this.edges = updateEdgeJson(this.edges, id, patch)
    this.commit(`edge:${id}:${Object.keys(patch).sort().join(',')}`)
  }

  /** 改节点编码;失败返回错误文案,数据不动 */
  renameNodeCode(from: string, to: string): string | null {
    const r = renameNodeCode(this.nodes, this.edges, from, to)
    if ('error' in r) return r.error
    this.nodes = r.nodes
    this.edges = r.edges
    this.commit()
    return null
  }

  /** 由面板发起的选中(如网关表单点某条出边);选中态经 bind 同步到画布 */
  selectEdge(id: string) {
    this.nodes = this.nodes.map((n) =>
      n.selected ? { ...n, selected: false } : n
    )
    this.edges = this.edges.map((e) =>
      e.selected !== (e.id === id) ? { ...e, selected: e.id === id } : e
    )
  }

  selectNode(id: string) {
    this.edges = this.edges.map((e) =>
      e.selected ? { ...e, selected: false } : e
    )
    this.nodes = this.nodes.map((n) =>
      n.selected !== (n.id === id) ? { ...n, selected: n.id === id } : n
    )
  }

  validate(): ValidationIssue[] {
    return validateDefinition(this.nodes, this.edges)
  }

  // ── 画布编辑:增删改都在这里落数据并提交历史 ───────────────────────────

  /** 追加一个节点并设为唯一选中(面板拖入落到空白处 / 点面板项 / 右键菜单添加) */
  addNode(node: FlowNode) {
    this.nodes = [...this.nodes, node]
    this.selectNode(node.id)
    this.commit()
  }

  /** 面板拖入落到跳转上;失败返回错误文案,数据不动 */
  insertNodeOnEdge(edgeId: string, node: FlowNode): string | null {
    const r = insertNodeOnEdge(this.nodes, this.edges, edgeId, node)
    if ('error' in r) return r.error
    this.nodes = r.nodes
    this.edges = r.edges
    this.selectNode(node.id)
    this.commit()
    return null
  }

  /** 复制选集到剪贴板;选集里没有可复制的节点返回 false */
  copy(): boolean {
    const clip = copySelection(this.nodes, this.edges)
    if (!clip) return false
    this.clipboard = clip
    return true
  }

  /** 粘贴到画布坐标 at(外接框左上角);剪贴板为空什么都不做 */
  paste(at: Point) {
    if (!this.clipboard) return
    const r = pasteClipboard(this.clipboard, this.nodes, this.edges, at)
    this.nodes = r.nodes
    this.edges = r.edges
    this.commit()
  }

  // ── 历史 ──────────────────────────────────────────────────────────────

  get canUndo() {
    return this.history.canUndo
  }

  get canRedo() {
    return this.history.canRedo
  }

  /**
   * 记录当前图为一步。画布侧在 xyflow 写完数据后调(dragstop / ondelete / onreconnect …),
   * 面板侧带合并键。选中态、视口不经这里,自然不进历史。
   */
  commit(coalesceKey?: string) {
    this.history.push({ nodes: this.nodes, edges: this.edges }, coalesceKey)
  }

  undo() {
    if (this.interacting) return
    const g = this.history.undo()
    if (g) this.restore(g)
  }

  redo() {
    if (this.interacting) return
    const g = this.history.redo()
    if (g) this.restore(g)
  }

  /**
   * 整图换成快照:快照里没有 selected,恢复后没有任何选中,面板回到流程概览。
   * measured 按 id 从当前节点合并回去——尺寸由 NODE_SIZE 写死不会变,带上它 xyflow
   * 就不必重测,节点不会在重测那一帧 visibility:hidden 闪一下。
   */
  private restore(g: Graph) {
    const measured = new Map(this.nodes.map((n) => [n.id, n.measured]))
    this.nodes = g.nodes.map((n) => {
      const m = measured.get(n.id)
      return m ? { ...n, measured: m } : n
    })
    this.edges = g.edges
  }

  async save() {
    this.saving = true
    try {
      const def = this.toDefJson()
      await designerApi.saveJson(def)
      this.savedSnapshot = JSON.stringify(def)
    } finally {
      this.saving = false
    }
  }
}
