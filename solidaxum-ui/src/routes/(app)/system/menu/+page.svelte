<script lang="ts">
  import { onMount } from 'svelte'
  import { menuApi, collectMenuTreeIds } from '$lib/api/system/menu.js'
  import type {
    MenuListVo,
    MenuCreateBo,
    MenuUpdateBo,
  } from '$lib/api/system/menu.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import DictTag from '$lib/components/DictTag.svelte'
  import FormDrawer from '$lib/components/FormDrawer.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import IconSelect from '$lib/components/IconSelect.svelte'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import TreeSelect from '$lib/components/TreeSelect.svelte'
  import { TreeTableShell, buildTree } from '$lib/components/crud/index.js'
  import { filterTreeNodes } from '$lib/components/crud/tree-filter.js'
  import { DICT_TAG_CLASS } from '$lib/components/dict-tag.js'
  import { toTreeSelectNodes } from '$lib/components/tree-select-utils.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'
  import { resolveMenuIcon } from '$lib/menu/icon-map.js'
  import { Plus, Pencil, Trash2 } from '@lucide/svelte'
  import type { ColumnDef } from '@tanstack/table-core'
  import { toast } from 'svelte-sonner'

  // ---- tree state ----
  let tree = $state<MenuListVo[]>([])
  let loading = $state(false)

  // SELECT_CLASS 与 role 页一致:统一原生 select 外观,匹配 shadcn input 高度/边框。
  const SELECT_CLASS =
    'border-input bg-background focus-visible:border-ring focus-visible:ring-ring/50 flex h-9 w-full rounded-md border px-3 py-1 text-sm shadow-xs outline-none focus-visible:ring-[3px]'

  async function load() {
    loading = true
    try {
      const resp = await menuApi.list()
      tree = resp.data ?? []
    } catch {
      // apiClient 已经 toast 错误,这里不重复提示
    } finally {
      loading = false
    }
  }

  onMount(() => {
    load()
  })

  // ---- create / edit dialog ----
  let dialogOpen = $state(false)
  let saving = $state(false)
  let editingId = $state<string | null>(null)
  const isEdit = $derived(editingId !== null)

  interface MenuFormModel {
    menuName: string
    parentId: string
    orderNum: number
    path: string
    component: string
    menuType: string
    visible: string
    status: string
    isFrame: string
    isCache: string
    perms: string
    icon: string
    remark: string
    queryParam: string
    activeMenu: string
  }

  function emptyForm(parentId = '0'): MenuFormModel {
    return {
      menuName: '',
      parentId,
      orderNum: 0,
      path: '',
      component: '',
      menuType: 'M',
      visible: '0',
      status: '0',
      isFrame: '1',
      isCache: '1',
      perms: '',
      icon: '',
      remark: '',
      queryParam: '',
      activeMenu: '',
    }
  }

  let form = $state<MenuFormModel>(emptyForm())

  function openCreate(parentId = '0') {
    editingId = null
    form = emptyForm(parentId)
    dialogOpen = true
  }

  function openEdit(row: MenuListVo) {
    editingId = row.menuId
    // 列表行字段与详情一致,直接用行数据预填(避免额外网络往返)
    form = {
      menuName: row.menuName ?? '',
      parentId: row.parentId ?? '0',
      orderNum: row.orderNum ?? 0,
      path: row.path ?? '',
      component: row.component ?? '',
      menuType: row.menuType ?? 'M',
      visible: row.visible ?? '0',
      status: row.status ?? '0',
      // 后端 0=是外链/缓存、1=否/不缓存(反直觉);行数据缺键时回退「否/不缓存」侧。
      isFrame: row.isFrame ?? '1',
      isCache: row.isCache ?? '1',
      perms: row.perms ?? '',
      icon: row.icon ?? '',
      remark: row.remark ?? '',
      queryParam: row.queryParam ?? '',
      activeMenu: row.activeMenu ?? '',
    }
    dialogOpen = true
  }

  async function handleConfirm() {
    if (!form.menuName) {
      toast.error('菜单名称不能为空')
      return
    }
    // orderNum 是 number,0 是合法排序值,只能判 null/undefined/NaN,不能用 !form.orderNum。
    if (
      form.orderNum === null ||
      form.orderNum === undefined ||
      Number.isNaN(form.orderNum)
    ) {
      toast.error('菜单顺序不能为空')
      return
    }
    // path 仅在「非按钮」时可见,按钮类型没有该字段故不校验 —— 对齐 Vue:
    // el-col v-if 会把 form-item 整个摘掉,element-plus 不校验未注册的字段。
    if (form.menuType !== 'F' && !form.path) {
      toast.error('路由地址不能为空')
      return
    }
    saving = true
    try {
      if (isEdit && editingId !== null) {
        const payload: MenuUpdateBo = {
          menuId: editingId,
          menuName: form.menuName,
          parentId: form.parentId,
          orderNum: form.orderNum,
          path: form.path,
          component: form.component || undefined,
          menuType: form.menuType,
          visible: form.visible,
          status: form.status,
          isFrame: form.isFrame,
          isCache: form.isCache,
          perms: form.perms || undefined,
          icon: form.icon || undefined,
          remark: form.remark || undefined,
          queryParam: form.queryParam || undefined,
          activeMenu: form.activeMenu || undefined,
        }
        await menuApi.update(payload)
        toast.success('修改成功')
      } else {
        const payload: MenuCreateBo = {
          menuName: form.menuName,
          parentId: form.parentId,
          orderNum: form.orderNum,
          path: form.path,
          component: form.component || undefined,
          menuType: form.menuType,
          visible: form.visible,
          status: form.status,
          isFrame: form.isFrame,
          isCache: form.isCache,
          perms: form.perms || undefined,
          icon: form.icon || undefined,
          remark: form.remark || undefined,
          queryParam: form.queryParam || undefined,
          activeMenu: form.activeMenu || undefined,
        }
        await menuApi.create(payload)
        toast.success('新增成功')
      }
      dialogOpen = false
      load()
    } catch {
      // apiClient 已 toast
    } finally {
      saving = false
    }
  }

  // ---- delete confirm ----
  let deleteOpen = $state(false)
  let deleting = $state(false)
  let deleteTarget = $state<MenuListVo | null>(null)

  // 有子节点的菜单单删会被后端 has_child 拒绝,须走 cascade 接口。
  const deleteHasChildren = $derived((deleteTarget?.children?.length ?? 0) > 0)
  // 确认弹窗文案按「有无子节点」分流:有子节点时明确警告将连同子孙一并删除。
  const deleteDesc = $derived(
    deleteHasChildren
      ? `「${deleteTarget?.menuName ?? ''}」下还有子菜单,删除将连同其全部子孙菜单一并删除,且不可恢复。确定继续?`
      : `确定要删除菜单「${deleteTarget?.menuName ?? ''}」吗?删除后不可恢复。`
  )

  function openDelete(row: MenuListVo) {
    deleteTarget = row
    deleteOpen = true
  }

  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      if (deleteHasChildren) {
        // Java 排除语义要求子孙同批在删除集合内,只传父 id 会被拒。
        await menuApi.cascadeDelete(collectMenuTreeIds(deleteTarget).join(','))
      } else {
        await menuApi.delete(deleteTarget.menuId)
      }
      toast.success('删除成功')
      deleteOpen = false
      deleteTarget = null
      load()
    } catch {
      // apiClient 已 toast
    } finally {
      deleting = false
    }
  }

  // ---- Vue 按 menuType(M/C/F) 与 visible 联动显隐 9 个字段(index.vue:156-330),逐条对齐。
  // 注意:隐藏只管 DOM,form 里的值仍会随 payload 发出去 —— Vue 也是这个行为,保持一致。
  const showForNonButton = $derived(form.menuType !== 'F') // icon / isFrame / path / visible
  const showForMenuOnly = $derived(form.menuType === 'C') // component / queryParam / isCache
  const showPerms = $derived(form.menuType !== 'M') // perms
  const showActiveMenu = $derived(form.visible !== '0') // activeMenu

  // ---- 类型列彩色 tag。分支顺序照 Vue getMenuTypeMeta(index.vue:434-445)。
  // 🔴 外链判定是 isFrame === '0' —— 本仓值域 0=是外链/1=非外链,与 Vue 的 'Y' 方向相反,
  //    写成 'Y' 会把全部菜单标成外链。
  function menuTypeMeta(r: MenuListVo): { label: string; cls: string } {
    if (r.menuType === 'F')
      return { label: '按钮', cls: DICT_TAG_CLASS.warning }
    if (r.isFrame === '0') return { label: '外链', cls: DICT_TAG_CLASS.danger }
    if (r.menuType === 'M')
      return { label: '目录', cls: DICT_TAG_CLASS.primary }
    return { label: '菜单', cls: DICT_TAG_CLASS.success }
  }

  // status 改走 cellRenderer 渲染 DictTag(值域对接 sys_normal_disable)。
  // menuType/menuName 也走 cellRenderer(彩色 tag / 内联图标)。
  const customCellIds = new Set(['status', 'menuType', 'menuName'])

  const columns: ColumnDef<MenuListVo>[] = [
    { id: 'menuName', accessorKey: 'menuName', header: '菜单名称' },
    { id: 'menuType', accessorKey: 'menuType', header: '类型' },
    { id: 'orderNum', accessorKey: 'orderNum', header: '排序' },
    {
      id: 'perms',
      accessorKey: 'perms',
      header: '权限标识',
      accessorFn: (r) => r.perms ?? '-',
    },
    {
      id: 'component',
      accessorKey: 'component',
      header: '组件路径',
      accessorFn: (r) => r.component ?? '-',
    },
    { id: 'status', accessorKey: 'status', header: '状态' },
    { id: 'actions', header: '操作' },
  ]

  // 后端 /system/menu/list 已返回组装好的 children;buildTree 兜底(幂等)。
  const fullTree = $derived(
    buildTree(tree as unknown as Record<string, unknown>[], {
      idKey: 'menuId',
      parentKey: 'parentId',
      rootValue: '0',
    }) as unknown as MenuListVo[]
  )

  // ---- Vue 有 menuName + status 两项筛选(index.vue:14-27)。
  // 本页是 TreeTableShell 全量树,不走分页请求 —— 故在前端过滤,不加 query 参数。
  let filters = $state<{ menuName?: string; status?: string }>({})

  const queryFields = [
    { key: 'menuName', label: '菜单名称', placeholder: '请输入菜单名称' },
    {
      key: 'status',
      label: '状态',
      type: 'select' as const,
      dictType: 'sys_normal_disable',
    },
  ]

  function handleSearch(model: Record<string, string>) {
    filters = { menuName: model.menuName, status: model.status }
  }
  function handleReset() {
    filters = {}
  }

  const treeData = $derived.by(() => {
    const name = (filters.menuName ?? '').trim()
    const status = filters.status ?? ''
    return filterTreeNodes(
      fullTree as unknown as Record<string, unknown>[],
      (n) =>
        (name === '' || String(n.menuName ?? '').includes(name)) &&
        (status === '' || n.status === status)
    ) as unknown as MenuListVo[]
  })

  // 上级菜单可选项:⚠️ 必须用未过滤的全量树 —— 否则一筛选,上级菜单可选项就跟着少了。
  const parentTreeData = $derived(
    toTreeSelectNodes(fullTree as unknown as Record<string, unknown>[], {
      idKey: 'menuId',
      labelKey: 'menuName',
    })
  )
</script>

<div class="space-y-4 p-4 md:p-6">
  <TreeTableShell
    data={treeData}
    {columns}
    isLoading={loading}
    error={null}
    treeColumnId="menuName"
    showSearch={false}
    onRefresh={load}
    emptyMessage="暂无菜单数据。"
    {customCellIds}
    title="菜单列表"
  >
    {#snippet toolbar()}
      <HasPermi perm="system:menu:add">
        <Button size="sm" onclick={() => openCreate('0')}>
          <Plus class="mr-1 h-4 w-4" />新增
        </Button>
      </HasPermi>
    {/snippet}
    {#snippet queryForm()}
      <QueryForm
        fields={queryFields}
        onSearch={handleSearch}
        onReset={handleReset}
      />
    {/snippet}
    {#snippet cellRenderer({ row, columnId })}
      {@const node = row as MenuListVo}
      {#if columnId === 'status'}
        <DictTag dictType="sys_normal_disable" value={node.status} />
      {:else if columnId === 'menuType'}
        {@const meta = menuTypeMeta(node)}
        <Badge variant="outline" class={meta.cls}>{meta.label}</Badge>
      {:else if columnId === 'menuName'}
        {@const Ico = node.icon ? resolveMenuIcon(node.icon) : null}
        <span class="flex items-center gap-2">
          {#if Ico}<Ico class="size-4 shrink-0" />{/if}
          <span class="truncate">{node.menuName}</span>
        </span>
      {:else}
        <div class="flex items-center gap-1">
          <HasPermi perm="system:menu:add">
            <Button
              variant="ghost"
              size="icon"
              title="新增子菜单"
              aria-label="新增子菜单"
              onclick={() => openCreate(node.menuId)}
            >
              <Plus class="size-4" />
            </Button>
          </HasPermi>
          <HasPermi perm="system:menu:edit">
            <Button
              variant="ghost"
              size="icon"
              title="编辑"
              aria-label="编辑"
              onclick={() => openEdit(node)}
            >
              <Pencil class="size-4" />
            </Button>
          </HasPermi>
          <HasPermi perm="system:menu:remove">
            <Button
              variant="ghost"
              size="icon"
              class="text-destructive hover:text-destructive"
              title="删除"
              aria-label="删除"
              onclick={() => openDelete(node)}
            >
              <Trash2 class="size-4" />
            </Button>
          </HasPermi>
        </div>
      {/if}
    {/snippet}
  </TreeTableShell>
</div>

<FormDrawer
  open={dialogOpen}
  onOpenChange={(o) => (dialogOpen = o)}
  title={isEdit ? '编辑菜单' : '新增菜单'}
  loading={saving}
  onConfirm={handleConfirm}
>
  <div class="flex flex-col gap-3">
    <Label for="menuName">菜单名称</Label>
    <Input
      id="menuName"
      bind:value={form.menuName}
      placeholder="请输入菜单名称"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label>上级菜单</Label>
    <TreeSelect
      treeData={parentTreeData}
      value={form.parentId}
      excludeIds={editingId !== null ? [editingId] : []}
      rootOption={{ id: '0', label: '主类目' }}
      placeholder="请选择上级菜单"
      onChange={(id) => {
        form.parentId = String(id)
      }}
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="orderNum">显示排序</Label>
    <Input
      id="orderNum"
      type="number"
      bind:value={form.orderNum}
      placeholder="请输入显示排序"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="menuType">菜单类型</Label>
    <select id="menuType" class={SELECT_CLASS} bind:value={form.menuType}>
      <option value="M">目录</option>
      <option value="C">菜单</option>
      <option value="F">按钮</option>
    </select>
  </div>
  {#if showForNonButton}
    <div class="flex flex-col gap-3">
      <Label for="path">路由地址</Label>
      <Input id="path" bind:value={form.path} placeholder="如 system/user" />
    </div>
  {/if}
  {#if showForMenuOnly}
    <div class="flex flex-col gap-3">
      <Label for="component">组件路径</Label>
      <Input
        id="component"
        bind:value={form.component}
        placeholder="如 /system/user/index (按钮类型可留空)"
      />
    </div>
  {/if}
  {#if showPerms}
    <div class="flex flex-col gap-3">
      <Label for="perms">权限标识</Label>
      <Input
        id="perms"
        bind:value={form.perms}
        placeholder="如 system:user:list"
      />
    </div>
  {/if}
  {#if showForNonButton}
    <div class="flex flex-col gap-3">
      <Label>菜单图标</Label>
      <IconSelect value={form.icon} onChange={(next) => (form.icon = next)} />
    </div>
  {/if}
  {#if showForNonButton}
    <div class="flex flex-col gap-3">
      <Label for="visible">是否显示</Label>
      <select id="visible" class={SELECT_CLASS} bind:value={form.visible}>
        <option value="0">显示</option>
        <option value="1">隐藏</option>
      </select>
    </div>
  {/if}
  <div class="flex flex-col gap-3">
    <Label for="status">状态</Label>
    <select id="status" class={SELECT_CLASS} bind:value={form.status}>
      <option value="0">正常</option>
      <option value="1">停用</option>
    </select>
  </div>
  {#if showForNonButton}
    <div class="flex flex-col gap-3">
      <Label for="isFrame">是否外链</Label>
      <select id="isFrame" class={SELECT_CLASS} bind:value={form.isFrame}>
        <option value="0">是</option>
        <option value="1">否</option>
      </select>
    </div>
  {/if}
  {#if showForMenuOnly}
    <div class="flex flex-col gap-3">
      <Label for="queryParam">
        路由参数
        <span class="text-muted-foreground text-xs">
          （访问路由的默认传递参数，如：{'{"id": 1, "name": "ry"}'}）
        </span>
      </Label>
      <Input
        id="queryParam"
        bind:value={form.queryParam}
        placeholder="请输入路由参数"
      />
    </div>
    <div class="flex flex-col gap-3">
      <Label for="isCache">是否缓存</Label>
      <select id="isCache" class={SELECT_CLASS} bind:value={form.isCache}>
        <option value="0">缓存</option>
        <option value="1">不缓存</option>
      </select>
    </div>
  {/if}
  {#if showActiveMenu}
    <div class="flex flex-col gap-3">
      <Label for="activeMenu">
        激活路由
        <span class="text-muted-foreground text-xs">
          （隐藏菜单填写默认激活路由，比如激活父菜单的路由 /system/user）
        </span>
      </Label>
      <Input
        id="activeMenu"
        bind:value={form.activeMenu}
        placeholder="请输入激活路径"
      />
    </div>
  {/if}
  <div class="flex flex-col gap-3">
    <Label for="remark">备注</Label>
    <Textarea
      id="remark"
      bind:value={form.remark}
      placeholder="请输入备注"
      rows={3}
    />
  </div>
</FormDrawer>

<ConfirmDialog
  open={deleteOpen}
  onOpenChange={(v) => (deleteOpen = v)}
  title="确认删除"
  desc={deleteDesc}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete}
/>
