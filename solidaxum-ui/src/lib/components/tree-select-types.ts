/** TreeSelect 节点(与 components/crud/tree.ts 的 buildTree 产物经 toTreeSelectNodes 映射后的结构)。 */
export type TreeSelectNode = {
  id: string
  label: string
  children?: TreeSelectNode[]
  disabled?: boolean
}
