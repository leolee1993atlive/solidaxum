<script lang="ts">
  import { onMount } from 'svelte'
  import { goto } from '$app/navigation'
  import { page } from '$app/state'
  import { genApi } from '$lib/api/tool/gen.js'
  import type { GenColumnRow, GenTableInfo } from '$lib/api/tool/gen.js'
  import { columnFieldOptions } from '$lib/api/tool/gen.js'
  import { validateGenTable } from '$lib/api/tool/gen-rules.js'
  import { dictApi } from '$lib/api/system/dict.js'
  import type { DictTypeOption } from '$lib/api/system/dict.js'
  import Main from '$lib/components/layout/Main.svelte'
  import TreeSelect from '$lib/components/TreeSelect.svelte'
  import { menuApi } from '$lib/api/system/menu.js'
  import { buildTree } from '$lib/components/crud/tree.js'
  import type { TreeSelectNode } from '$lib/components/tree-select-types.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import { toast } from 'svelte-sonner'

  const tableId = page.params.tableId
  let info = $state<GenTableInfo>({})
  let rows = $state<GenColumnRow[]>([])
  let loading = $state(true)
  let saving = $state(false)

  // GE12:回列表时带回原分页(对齐 Vue editTable.vue:197-202 的 close())。
  // 写成普通函数而不是 $derived —— page.url 本身是响应式的,调用处每次读到的都是新值,
  // 不需要再包一层派生。
  function backHref(): string {
    const p = page.url.searchParams.get('pageNum')
    return p ? `/tool/gen?pageNum=${p}` : '/tool/gen'
  }

  // 逐个抄自 Vue editTable.vue,不要自拟取值
  const JAVA_TYPES = ['String', 'Integer', 'Long', 'Double', 'BigDecimal', 'LocalDateTime', 'Boolean']
  const QUERY_TYPES = [
    { v: 'EQ', label: '=' },
    { v: 'NE', label: '!=' },
    { v: 'GT', label: '>' },
    { v: 'GE', label: '>=' },
    { v: 'LT', label: '<' },
    { v: 'LE', label: '<=' },
    { v: 'LIKE', label: 'LIKE' },
    { v: 'BETWEEN', label: 'BETWEEN' },
  ]
  const HTML_TYPES = [
    { v: 'input', label: '文本框' },
    { v: 'inputNumber', label: '数字框' },
    { v: 'textarea', label: '文本域' },
    { v: 'select', label: '下拉框' },
    { v: 'radio', label: '单选框' },
    { v: 'checkbox', label: '复选框' },
    { v: 'switch', label: '开关' },
    { v: 'datetime', label: '日期控件' },
    { v: 'imageUpload', label: '图片上传' },
    { v: 'fileUpload', label: '文件上传' },
    { v: 'editor', label: '富文本控件' },
  ]
  /** 只有这些显示类型才需要字典(对齐 Vue 的联动禁用) */
  const DICT_HTML_TYPES = new Set(['select', 'radio', 'checkbox'])

  let dictTypes = $state<DictTypeOption[]>([])
  let menuNodes = $state<TreeSelectNode[]>([])

  const fieldOptions = $derived(columnFieldOptions(rows))

  /**
   * 由文本框直接编辑的 GenTableInfo 字段名。
   * ⚠️ 写成字面量联合而不是 string:GenTableInfo 已去掉 `[k: string]: unknown`,
   *    动态下标必须落在真实键上 —— 键名拼错当场报错,而不是静默读到 undefined
   *    (那正是 ProfileDropdown 的 username/userName 常年没人发现的成因)。
   */
  type GenTextKey =
    | 'tableName'
    | 'tableComment'
    | 'className'
    | 'functionAuthor'
    | 'remark'
    | 'packageName'
    | 'moduleName'
    | 'businessName'
    | 'functionName'

  /** 树表配置里「从本表字段中选一个」的那几个 GenTableInfo 键。 */
  type GenTreeKey =
    | 'treeCode'
    | 'treeParentCode'
    | 'treeName'
    | 'treeAncestorsField'
    | 'treeOrderField'

  function s(key: GenTextKey): string {
    return info[key] ?? ''
  }
  function setS(key: GenTextKey, v: string) {
    info = { ...info, [key]: v }
  }

  /** 基本信息 Tab 的文本字段。提到 script 里是为了让 key 保持字面量类型。 */
  const BASIC_FIELDS: [key: GenTextKey, label: string][] = [
    ['tableName', '表名称'],
    ['tableComment', '表描述'],
    ['className', '实体类名称'],
    ['functionAuthor', '作者'],
  ]

  /** 生成信息 Tab 的文本字段。同上,提出来保住字面量类型。 */
  const GEN_FIELDS: [key: GenTextKey, label: string][] = [
    ['moduleName', '生成模块名'],
    ['businessName', '生成业务名'],
    ['functionName', '生成功能名'],
  ]

  /** 树表配置字段(GE6)。 */
  const TREE_FIELDS: { key: GenTreeKey; label: string; hint: string }[] = [
    { key: 'treeCode', label: '树编码字段', hint: '树显示的编码字段名，如 dept_id' },
    { key: 'treeParentCode', label: '树父编码字段', hint: '树显示的父编码字段名，如 parent_id' },
    { key: 'treeName', label: '树名称字段', hint: '树节点的显示名称字段名，如 dept_name' },
    { key: 'treeAncestorsField', label: '祖级字段', hint: '存放祖级链的字段，如 ancestors' },
    { key: 'treeOrderField', label: '树排序字段', hint: '同级节点排序字段，如 order_num' },
  ]

  // 复选框 '0'/'1' 互转
  function flag(row: GenColumnRow, key: 'isInsert' | 'isEdit' | 'isList' | 'isQuery' | 'isRequired'): boolean {
    return row[key] === '1'
  }
  function setFlag(
    row: GenColumnRow,
    key: 'isInsert' | 'isEdit' | 'isList' | 'isQuery' | 'isRequired',
    v: boolean
  ) {
    row[key] = v ? '1' : '0'
    rows = [...rows]
  }

  async function loadMenuTree() {
    const list = await menuApi.list()
    const listData = list.data ?? []
    const tree = buildTree(listData as unknown as Record<string, unknown>[], {
      idKey: 'menuId',
      parentKey: 'parentId',
      rootValue: '0',
    })
    const toNodes = (items: Record<string, unknown>[]): TreeSelectNode[] =>
      items.map((m) => ({
        id: String(m.menuId),
        label: String(m.menuName),
        children: m.children ? toNodes(m.children as Record<string, unknown>[]) : undefined,
      }))
    menuNodes = [{ id: '0', label: '主类目', children: toNodes(tree) }]
  }

  onMount(async () => {
    if (!tableId) {
      toast.error('缺少表 ID')
      goto('/tool/gen')
      return
    }
    try {
      const r = await genApi.detail(tableId)
      info = { ...(r.data?.info ?? {}), tableId }
      // 对齐 Vue editTable.vue:230 —— 老数据可能没有这个字段,
      // 而后端 GenTableUpdateBo.frontend_type 是必填非 Option,空串会 400。
      if (!info.frontendType) info = { ...info, frontendType: 'vue' }
      rows = (r.data?.rows ?? []) as GenColumnRow[]
      dictTypes = (await dictApi.optionselect()).data ?? []
      await loadMenuTree()
    } catch (e) {
      console.error(e)
      toast.error(e instanceof Error ? e.message : '加载详情失败')
    } finally {
      loading = false
    }
  })

  async function save() {
    // GE8:提交前跑完整校验(取代原来只查树表三项的 missingTreeFields)。
    // 规则逐条移植 Vue,见 gen-rules.ts。
    const errors = validateGenTable(info as Record<string, unknown>)
    if (errors.length > 0) {
      toast.error(`表单校验未通过：${errors.map((e) => e.message).join('、')}`)
      return
    }
    saving = true
    try {
      await genApi.save({ ...info, columns: rows })
      toast.success('保存成功')
      goto(backHref())
    } catch (e) {
      console.error(e)
      toast.error(e instanceof Error ? e.message : '保存失败')
    } finally {
      saving = false
    }
  }
</script>

<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  {#if loading}
    <div class="text-muted-foreground py-8 text-center">加载中...</div>
  {:else}
    <!-- GE10:Tab 顺序对齐 Vue editTable.vue:3-119(基本 / 字段 / 生成),
         且默认打开中间的「字段信息」(activeName = ref('columnInfo'),editTable.vue:141)。 -->
    <Tabs.Root value="columns">
      <Tabs.List>
        <Tabs.Trigger value="basic">基本信息</Tabs.Trigger>
        <Tabs.Trigger value="columns">字段信息</Tabs.Trigger>
        <Tabs.Trigger value="gen">生成信息</Tabs.Trigger>
      </Tabs.List>

      <!-- 基本信息 Tab -->
      <Tabs.Content value="basic">
        <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
          {#each BASIC_FIELDS as [k, label]}
            <div class="flex flex-col gap-2">
              <Label for={k}>{label}</Label>
              <Input
                id={k}
                value={s(k)}
                oninput={(e) => setS(k, (e.target as HTMLInputElement).value)}
              />
            </div>
          {/each}
          <!-- GE7:Vue basicInfoForm.vue:24-28 有「备注」多行文本域,占整行 -->
          <div class="flex flex-col gap-2 md:col-span-2">
            <Label for="remark">备注</Label>
            <textarea
              id="remark"
              rows="3"
              class="border-input focus-visible:border-ring focus-visible:ring-ring/50 w-full rounded-md border px-3 py-2 text-sm shadow-xs outline-none focus-visible:ring-[3px]"
              value={s('remark')}
              oninput={(e) => setS('remark', (e.target as HTMLTextAreaElement).value)}
            ></textarea>
          </div>
        </div>
      </Tabs.Content>

      <!-- 字段信息 Tab (GE1/GE9) -->
      <Tabs.Content value="columns">
        <div class="overflow-x-auto">
          <table class="w-full min-w-[1400px] border-collapse text-sm">
            <thead class="bg-muted">
              <tr>
                <th class="border p-2 text-left">字段列名</th>
                <th class="border p-2 text-left">字段描述</th>
                <th class="border p-2 text-left">物理类型</th>
                <th class="border p-2 text-left">Java 类型</th>
                <th class="border p-2 text-left">Java 属性</th>
                <th class="border p-2">插入</th>
                <th class="border p-2">编辑</th>
                <th class="border p-2">列表</th>
                <th class="border p-2">查询</th>
                <th class="border p-2 text-left">查询方式</th>
                <th class="border p-2">必填</th>
                <th class="border p-2 text-left">显示类型</th>
                <th class="border p-2 text-left">字典类型</th>
              </tr>
            </thead>
            <tbody>
              {#each rows as row (row.columnId)}
                <tr>
                  <td class="border p-2">{row.columnName}</td>
                  <td class="border p-1"><Input bind:value={row.columnComment} /></td>
                  <td class="border p-2">{row.columnType}</td>
                  <td class="border p-1">
                    <select class="w-full rounded border px-2 py-1" bind:value={row.javaType}>
                      {#each JAVA_TYPES as t (t)}<option value={t}>{t}</option>{/each}
                    </select>
                  </td>
                  <td class="border p-1"><Input bind:value={row.javaField} /></td>
                  {#each ['isInsert', 'isEdit', 'isList', 'isQuery'] as k (k)}
                    <td class="border p-2 text-center">
                      <input
                        type="checkbox"
                        checked={flag(row, k as 'isInsert')}
                        onchange={(e) => setFlag(row, k as 'isInsert', e.currentTarget.checked)}
                      />
                    </td>
                  {/each}
                  <td class="border p-1">
                    <select class="w-full rounded border px-2 py-1" bind:value={row.queryType}>
                      {#each QUERY_TYPES as q (q.v)}<option value={q.v}>{q.label}</option>{/each}
                    </select>
                  </td>
                  <td class="border p-2 text-center">
                    <input
                      type="checkbox"
                      checked={flag(row, 'isRequired')}
                      onchange={(e) => setFlag(row, 'isRequired', e.currentTarget.checked)}
                    />
                  </td>
                  <td class="border p-1">
                    <select class="w-full rounded border px-2 py-1" bind:value={row.htmlType}>
                      {#each HTML_TYPES as h (h.v)}<option value={h.v}>{h.label}</option>{/each}
                    </select>
                  </td>
                  <td class="border p-1">
                    <select
                      class="w-full rounded border px-2 py-1"
                      bind:value={row.dictType}
                      disabled={!DICT_HTML_TYPES.has(row.htmlType)}
                    >
                      <option value="">无</option>
                      {#each dictTypes as d (d.dictType)}
                        <option value={d.dictType}>{d.dictName} ({d.dictType})</option>
                      {/each}
                    </select>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </Tabs.Content>

      <!-- 生成信息 Tab (GE2/GE4/GE5/GE6) -->
      <Tabs.Content value="gen">
        <div class="grid gap-6">
          <!-- 生成模板 GE2 -->
          <div class="grid gap-2 sm:max-w-md">
            <Label>生成模板</Label>
            <select class="rounded border px-2 py-1" bind:value={info.tplCategory}>
              <option value="crud">单表（增删改查）</option>
              <option value="tree">树表（增删改查）</option>
            </select>
          </div>

          <!-- 前端模板 GE3 -->
          <div class="grid gap-2 sm:max-w-md">
            <Label>前端模板</Label>
            <select class="rounded border px-2 py-1" bind:value={info.frontendType}>
              <option value="vue">Vue</option>
              <option value="svelte">Svelte</option>
            </select>
            <span class="text-muted-foreground text-xs">
              对应后端模板目录。⚠️ 本仓只有 vue / svelte 两套
              —— Vue 版列的 react 在这里没有对应模板，选了会静默生成 Vue 代码，故不提供。
            </span>
          </div>

          <!-- 生成包路径 GE3 -->
          <div class="grid gap-2 sm:max-w-md">
            <Label for="packageName">生成包路径</Label>
            <Input
              id="packageName"
              value={s('packageName')}
              oninput={(e) => setS('packageName', (e.target as HTMLInputElement).value)}
            />
            <span class="text-muted-foreground text-xs">生成代码所在的模块路径</span>
          </div>

          <!-- 上级菜单 GE4 -->
          <div class="grid gap-2 sm:max-w-md">
            <Label>上级菜单</Label>
            <TreeSelect treeData={menuNodes} value={info.parentMenuId ?? undefined} placeholder="请选择上级菜单" onChange={(id) => { info = { ...info, parentMenuId: String(id) } }} />
            <span class="text-muted-foreground text-xs">
              生成菜单 SQL 时作为 parent_id 写入
            </span>
          </div>

          <!-- 增强选项 GE5 -->
          <h4 class="border-b pb-1 font-medium">增强选项</h4>
          <div class="grid gap-4">
            <label class="flex items-center gap-2 text-sm">
              <input type="checkbox" bind:checked={info.enableExport} />
              导出能力
              <span class="text-muted-foreground text-xs">关闭后不生成 export 接口与前端导出按钮</span>
            </label>

            <div class="grid gap-2">
              <label class="flex items-center gap-2 text-sm">
                <input type="checkbox" bind:checked={info.enableStatus} />
                状态切换
                <span class="text-muted-foreground text-xs">生成 changeStatus 接口与列表状态开关列</span>
              </label>
              {#if info.enableStatus}
                <select class="rounded border px-2 py-1 sm:max-w-md" bind:value={info.statusField}>
                  <option value="">请选择状态字段</option>
                  {#each fieldOptions as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
                </select>
              {/if}
            </div>

            <div class="grid gap-2">
              <label class="flex items-center gap-2 text-sm">
                <input type="checkbox" bind:checked={info.enableUnique} />
                组合唯一校验
                <span class="text-muted-foreground text-xs">按选中字段生成组合唯一校验，新增和修改都会校验</span>
              </label>
              {#if info.enableUnique}
                <select class="rounded border px-2 py-1 sm:max-w-md" multiple size="5" bind:value={info.uniqueFields}>
                  {#each fieldOptions as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
                </select>
              {/if}
            </div>

            <div class="grid gap-2">
              <label class="flex items-center gap-2 text-sm">
                <input type="checkbox" bind:checked={info.enableSort} />
                排序调整
                <span class="text-muted-foreground text-xs">生成 updateSort 接口，列表中以输入框快速调整排序</span>
              </label>
              {#if info.enableSort}
                <select class="rounded border px-2 py-1 sm:max-w-md" bind:value={info.sortField}>
                  <option value="">请选择排序字段</option>
                  {#each fieldOptions as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
                </select>
              {/if}
            </div>
          </div>

          <!-- 树表配置 GE6 -->
          {#if info.tplCategory === 'tree'}
            <h4 class="border-b pb-1 font-medium">树表信息</h4>
            <div class="grid gap-4 sm:grid-cols-2">
              {#each TREE_FIELDS as f (f.key)}
                <div class="grid gap-2">
                  <Label>{f.label}</Label>
                  <select class="rounded border px-2 py-1" bind:value={info[f.key]}>
                    <option value="">请选择</option>
                    {#each fieldOptions as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
                  </select>
                  <span class="text-muted-foreground text-xs">{f.hint}</span>
                </div>
              {/each}
              <div class="grid gap-2">
                <Label>根节点值</Label>
                <Input bind:value={info.treeRootValue} placeholder="顶层节点的父值，如 0" />
              </div>
            </div>
          {/if}

          <!-- 生成模块名等。GE13:删掉「生成路径」(genPath)输入框 —— Vue 表单已不暴露该字段。
               ⚠️ 字段本身仍随 {...info} 原值透传给后端(gen_path: Option<String>),不会丢。 -->
          <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
            {#each GEN_FIELDS as [k, label]}
              <div class="flex flex-col gap-2">
                <Label for={k}>{label}</Label>
                <Input
                  id={k}
                  value={s(k)}
                  oninput={(e) => setS(k, (e.target as HTMLInputElement).value)}
                />
              </div>
            {/each}
          </div>
        </div>
      </Tabs.Content>
    </Tabs.Root>

    <!-- GE11:按钮在全部 Tab 下方居中(对齐 Vue editTable.vue:120-125) -->
    <div class="flex justify-center gap-2 pt-6">
      <Button disabled={saving || loading} onclick={save}>提交</Button>
      <Button variant="outline" onclick={() => goto(backHref())}>返回</Button>
    </div>
  {/if}
</Main>
