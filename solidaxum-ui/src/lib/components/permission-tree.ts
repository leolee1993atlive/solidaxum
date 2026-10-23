import type { MenuTreeSelectVo } from '$lib/api/system/menu.js'
import type { TreeSelectNode } from './tree-select-types.js'

/** 按钮权限(F 类菜单)。渲染在所属菜单行右侧,不作为树节点。 */
export type ButtonPermission = {
  id: string
  label: string
  disabled: boolean
}

/** 权限树节点(M 目录 / C 菜单)。数据权限的部门树复用同一结构(buttons 为空)。 */
export type PermissionNode = {
  id: string
  label: string
  /** 自身停用或祖先目录停用 → 不可勾选 */
  disabled: boolean
  /** visible === '1',仅显示一个「隐藏」标记 */
  hidden: boolean
  buttons: ButtonPermission[]
  children: PermissionNode[]
}

export type ToggleOptions = {
  /** true = 父子联动(对齐 Vue form.menuCheckStrictly,注意与 el-tree 的 check-strictly 语义相反) */
  linked: boolean
}

const MENU_TYPE_BUTTON = 'F'
const MENU_TYPE_DIR = 'M'
const STATUS_DISABLED = '1'
const VISIBLE_HIDDEN = '1'

/**
 * 后端菜单树 → 权限树。
 * 对齐 Vue `buildRoleMenuPermissionOptions`:F 类抽成父节点的 buttons;
 * 停用只经由**目录**向下传染(停用的菜单只禁自身与自身按钮)。
 */
export function buildPermissionTree(
  menus: MenuTreeSelectVo[],
  ancestorDirDisabled = false
): PermissionNode[] {
  const nodes: PermissionNode[] = []
  for (const menu of menus) {
    if (menu.menuType === MENU_TYPE_BUTTON) continue
    const selfDisabled = menu.status === STATUS_DISABLED
    const nodeDisabled = ancestorDirDisabled || selfDisabled
    const nextDirDisabled =
      ancestorDirDisabled || (menu.menuType === MENU_TYPE_DIR && selfDisabled)
    const children = menu.children ?? []
    nodes.push({
      id: String(menu.id),
      label: menu.label,
      disabled: nodeDisabled,
      hidden: menu.visible === VISIBLE_HIDDEN,
      buttons: children
        .filter((c) => c.menuType === MENU_TYPE_BUTTON)
        .map((b) => ({
          id: String(b.id),
          label: b.label,
          disabled: nodeDisabled || b.status === STATUS_DISABLED,
        })),
      children: buildPermissionTree(children, nextDirDisabled),
    })
  }
  return nodes
}

/** 部门树(id/label/children)→ 权限树节点,供数据权限 Tab 复用 PermissionTree。 */
export function fromTreeSelectNodes(nodes: TreeSelectNode[]): PermissionNode[] {
  return nodes.map((n) => ({
    id: String(n.id),
    label: n.label,
    disabled: false,
    hidden: false,
    buttons: [],
    children: n.children ? fromTreeSelectNodes(n.children) : [],
  }))
}

/** 节点自身 + 全部子孙节点 id(不含按钮)。 */
export function collectNodeIds(
  nodes: PermissionNode[],
  includeDisabled = true
): string[] {
  return nodes.flatMap((n) => [
    ...(includeDisabled || !n.disabled ? [n.id] : []),
    ...collectNodeIds(n.children, includeDisabled),
  ])
}

/** 全部按钮 id。 */
export function collectButtonIds(
  nodes: PermissionNode[],
  includeDisabled = true
): string[] {
  return nodes.flatMap((n) => [
    ...n.buttons
      .filter((b) => includeDisabled || !b.disabled)
      .map((b) => b.id),
    ...collectButtonIds(n.children, includeDisabled),
  ])
}

/** 全部可勾选 id(节点 + 按钮,排除 disabled) —— 「全选/全不选」用。 */
export function allSelectableIds(nodes: PermissionNode[]): string[] {
  return [...collectNodeIds(nodes, false), ...collectButtonIds(nodes, false)]
}

export function isAllSelected(
  nodes: PermissionNode[],
  checked: ReadonlySet<string>
): boolean {
  const ids = allSelectableIds(nodes)
  return ids.length > 0 && ids.every((id) => checked.has(id))
}

function findNode(
  nodes: PermissionNode[],
  id: string
): PermissionNode | undefined {
  for (const n of nodes) {
    if (n.id === id) return n
    const hit = findNode(n.children, id)
    if (hit) return hit
  }
  return undefined
}

function findButton(
  nodes: PermissionNode[],
  id: string
): ButtonPermission | undefined {
  for (const n of nodes) {
    const hit = n.buttons.find((b) => b.id === id)
    if (hit) return hit
    const deep = findButton(n.children, id)
    if (deep) return deep
  }
  return undefined
}

/** 子树内全部可勾选 id(含自身),排除 disabled。 */
function selectableSubtreeIds(node: PermissionNode): string[] {
  return [
    ...(node.disabled ? [] : [node.id]),
    ...node.buttons.filter((b) => !b.disabled).map((b) => b.id),
    ...node.children.flatMap(selectableSubtreeIds),
  ]
}

/** id → 祖先节点 id 链(自底向上,不含自身)。按钮 id 亦可查,链首为其所属菜单。 */
export function buildAncestorMap(
  nodes: PermissionNode[]
): Map<string, string[]> {
  const map = new Map<string, string[]>()
  const walk = (list: PermissionNode[], ancestors: string[]) => {
    for (const n of list) {
      map.set(n.id, ancestors)
      const own = [n.id, ...ancestors]
      for (const b of n.buttons) map.set(b.id, own)
      walk(n.children, own)
    }
  }
  walk(nodes, [])
  return map
}

/**
 * 自底向上重算各节点自身的勾选态:
 * 「全部可用子节点与可用按钮都选中」→ 节点进集合;否则移出(即半选)。
 * 只在**用户操作后**调用 —— 回填时不调,否则会把后端存的半选父节点抹掉。
 */
function reconcile(
  nodes: PermissionNode[],
  checked: Set<string>
): Set<string> {
  const visit = (node: PermissionNode) => {
    node.children.forEach(visit)
    const childIds = [
      ...node.children.filter((c) => !c.disabled).map((c) => c.id),
      ...node.buttons.filter((b) => !b.disabled).map((b) => b.id),
    ]
    // 无可用子项的叶子节点:保持用户的显式勾选,不参与推导。
    if (childIds.length === 0) return
    if (childIds.every((id) => checked.has(id))) checked.add(node.id)
    else checked.delete(node.id)
  }
  nodes.forEach(visit)
  return checked
}

/** 勾选/取消一个菜单节点。 */
export function toggleNode(
  nodes: PermissionNode[],
  checked: ReadonlySet<string>,
  id: string,
  next: boolean,
  { linked }: ToggleOptions
): Set<string> {
  const out = new Set(checked)
  const node = findNode(nodes, id)
  if (!node || node.disabled) return out
  if (!linked) {
    if (next) out.add(id)
    else out.delete(id)
    return out
  }
  for (const sid of selectableSubtreeIds(node)) {
    if (next) out.add(sid)
    else out.delete(sid)
  }
  return reconcile(nodes, out)
}

/** 勾选/取消一个按钮权限。 */
export function toggleButton(
  nodes: PermissionNode[],
  checked: ReadonlySet<string>,
  buttonId: string,
  next: boolean,
  { linked }: ToggleOptions
): Set<string> {
  const out = new Set(checked)
  const button = findButton(nodes, buttonId)
  if (!button || button.disabled) return out
  if (next) out.add(buttonId)
  else out.delete(buttonId)
  return linked ? reconcile(nodes, out) : out
}

function hasCheckedDescendant(
  node: PermissionNode,
  checked: ReadonlySet<string>
): boolean {
  if (node.buttons.some((b) => checked.has(b.id))) return true
  return node.children.some(
    (c) => checked.has(c.id) || hasCheckedDescendant(c, checked)
  )
}

export type NodeCheckState = 'checked' | 'indeterminate' | 'unchecked'

export function nodeCheckState(
  node: PermissionNode,
  checked: ReadonlySet<string>
): NodeCheckState {
  if (checked.has(node.id)) return 'checked'
  return hasCheckedDescendant(node, checked) ? 'indeterminate' : 'unchecked'
}

/**
 * 提交给后端的 id 集合。
 * - 联动开:选中集 ∪ 其全部祖先(等价于 el-tree 的 checkedKeys + halfCheckedKeys)。
 * - 联动关:选中集 ∪ **按钮**的祖先链(对齐 Vue getDerivedMenuIdsFromButtons);菜单的祖先不补。
 */
export function submitIds(
  nodes: PermissionNode[],
  checked: ReadonlySet<string>,
  { linked }: ToggleOptions
): string[] {
  const ancestors = buildAncestorMap(nodes)
  const buttonIds = new Set(collectButtonIds(nodes))
  const out = new Set<string>(checked)
  for (const id of checked) {
    if (!linked && !buttonIds.has(id)) continue
    for (const a of ancestors.get(id) ?? []) out.add(a)
  }
  return [...out].sort((a, b) => a.length - b.length || a.localeCompare(b))
}
