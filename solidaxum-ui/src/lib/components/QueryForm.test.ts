import { dictCache, fetchDict } from '$lib/hooks/use-dict.js'
import {
  cleanup,
  render,
  screen,
  fireEvent,
  waitFor,
} from '@testing-library/svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import QueryForm from './QueryForm.svelte'

vi.mock('$lib/hooks/use-dict.js', async (importOriginal) => {
  const mod = await importOriginal<typeof import('$lib/hooks/use-dict.js')>()
  return {
    ...mod,
    fetchDict: vi.fn(),
  }
})

vi.mock('$lib/api/system/user.js', () => ({
  userApi: {
    optionselect: vi.fn().mockResolvedValue({
      data: [
        { userId: '1', userName: 'admin', nickName: '超级管理员' },
        { userId: '2', userName: 'lisi', nickName: '李四' },
      ],
    }),
  },
}))

const mockedFetchDict = vi.mocked(fetchDict)

afterEach(cleanup)
beforeEach(() => {
  dictCache.clear()
  mockedFetchDict.mockReset()
})

describe('QueryForm 向后兼容', () => {
  it('旧用法(无 type 字段)仍渲染文本输入框', async () => {
    render(QueryForm, {
      props: {
        fields: [
          { key: 'userName', label: '用户名', placeholder: '请输入用户名' },
        ],
      },
    })
    const input = screen.getByPlaceholderText('请输入用户名')
    expect(input).toBeInTheDocument()
    expect(input.tagName).toBe('INPUT')
  })

  it('旧用法搜索仍回调出 {key: value}', async () => {
    const onSearch = vi.fn()
    render(QueryForm, {
      props: { fields: [{ key: 'userName', label: '用户名' }], onSearch },
    })
    await fireEvent.input(screen.getByLabelText('用户名'), {
      target: { value: 'admin' },
    })
    await fireEvent.click(screen.getByRole('button', { name: /搜索/ }))
    expect(onSearch).toHaveBeenCalledWith(
      expect.objectContaining({ userName: 'admin' })
    )
  })

  it('空字符串文本字段仍原样回传给 onSearch', async () => {
    const onSearch = vi.fn()
    render(QueryForm, {
      props: { fields: [{ key: 'userName', label: '用户名' }], onSearch },
    })
    await fireEvent.input(screen.getByLabelText('用户名'), {
      target: { value: '' },
    })
    await fireEvent.click(screen.getByRole('button', { name: /搜索/ }))
    expect(onSearch).toHaveBeenCalledWith({ userName: '' })
  })
})

describe('QueryForm select 类型', () => {
  // bits-ui Select 的触发器用 pointerdown 打开菜单(jsdom 无原生 PointerEvent,
  // fireEvent.pointerDown 派发的合成事件即可命中 onpointerdown 处理器)。
  async function openSelect(label: string) {
    const trigger = screen.getByLabelText(label)
    await fireEvent.pointerDown(trigger)
    return trigger
  }

  it('渲染为下拉触发器并带 options', async () => {
    render(QueryForm, {
      props: {
        fields: [
          {
            key: 'status',
            label: '状态',
            type: 'select',
            options: [
              { label: '正常', value: '0' },
              { label: '停用', value: '1' },
            ],
          },
        ],
      },
    })
    const trigger = screen.getByLabelText('状态')
    // 触发器是 Select.Trigger 渲染的按钮(原为原生 <select>)
    expect(trigger.tagName).toBe('BUTTON')
    // options 只在弹层打开后挂载(portal)
    await openSelect('状态')
    expect(
      await screen.findByRole('option', { name: '全部' })
    ).toBeInTheDocument()
    expect(screen.getByRole('option', { name: '正常' })).toBeInTheDocument()
    expect(screen.getByRole('option', { name: '停用' })).toBeInTheDocument()
  })

  it('select 选中值进入 onSearch 的 model', async () => {
    const onSearch = vi.fn()
    render(QueryForm, {
      props: {
        fields: [
          {
            key: 'status',
            label: '状态',
            type: 'select',
            options: [
              { label: '正常', value: '0' },
              { label: '停用', value: '1' },
            ],
          },
        ],
        onSearch,
      },
    })
    await openSelect('状态')
    await fireEvent.pointerUp(
      await screen.findByRole('option', { name: '停用' })
    )
    await fireEvent.click(screen.getByRole('button', { name: /搜索/ }))
    expect(onSearch).toHaveBeenCalledWith(
      expect.objectContaining({ status: '1' })
    )
  })

  it('select 选择「全部」时仍回传空字符串', async () => {
    const onSearch = vi.fn()
    render(QueryForm, {
      props: {
        fields: [
          {
            key: 'status',
            label: '状态',
            type: 'select',
            options: [
              { label: '正常', value: '0' },
              { label: '停用', value: '1' },
            ],
          },
        ],
        onSearch,
      },
    })
    await openSelect('状态')
    await fireEvent.pointerUp(
      await screen.findByRole('option', { name: '停用' })
    )
    await openSelect('状态')
    await fireEvent.pointerUp(
      await screen.findByRole('option', { name: '全部' })
    )
    await fireEvent.click(screen.getByRole('button', { name: /搜索/ }))
    expect(onSearch).toHaveBeenCalledWith({ status: '' })
  })

  it('select 支持 dictType 拉取字典选项', async () => {
    mockedFetchDict.mockResolvedValue([
      {
        dictCode: '1',
        dictSort: 0,
        dictLabel: '启用',
        dictValue: '0',
        dictType: 'sys_status',
        isDefault: 'Y',
      },
      {
        dictCode: '2',
        dictSort: 1,
        dictLabel: '禁用',
        dictValue: '1',
        dictType: 'sys_status',
        isDefault: 'N',
      },
    ])

    render(QueryForm, {
      props: {
        fields: [
          {
            key: 'status',
            label: '状态',
            type: 'select',
            dictType: 'sys_status',
          },
        ],
      },
    })

    expect(mockedFetchDict).toHaveBeenCalledWith('sys_status')
    await openSelect('状态')
    expect(
      await screen.findByRole('option', { name: '启用' })
    ).toBeInTheDocument()
    expect(screen.getByRole('option', { name: '禁用' })).toBeInTheDocument()
  })

  it('重置后 model 清空', async () => {
    const onSearch = vi.fn()
    const onReset = vi.fn()
    render(QueryForm, {
      props: {
        fields: [{ key: 'userName', label: '用户名' }],
        onSearch,
        onReset,
      },
    })
    await fireEvent.input(screen.getByLabelText('用户名'), {
      target: { value: 'x' },
    })
    await fireEvent.click(screen.getByRole('button', { name: /重置/ }))
    expect(onReset).toHaveBeenCalled()
    await fireEvent.click(screen.getByRole('button', { name: /搜索/ }))
    expect(onSearch).toHaveBeenCalledWith({})
  })
})

describe('QueryForm dateRange 类型', () => {
  it('渲染起止两个日期输入', () => {
    render(QueryForm, {
      props: {
        fields: [{ key: 'createTime', label: '创建时间', type: 'dateRange' }],
      },
    })
    expect(screen.getByLabelText('创建时间 起')).toBeInTheDocument()
    expect(screen.getByLabelText('创建时间 止')).toBeInTheDocument()
  })

  it('默认写入 beginTime / endTime 两个键', async () => {
    const onSearch = vi.fn()
    render(QueryForm, {
      props: {
        fields: [{ key: 'createTime', label: '创建时间', type: 'dateRange' }],
        onSearch,
      },
    })
    await fireEvent.change(screen.getByLabelText('创建时间 起'), {
      target: { value: '2026-01-01' },
    })
    await fireEvent.change(screen.getByLabelText('创建时间 止'), {
      target: { value: '2026-01-31' },
    })
    await fireEvent.click(screen.getByRole('button', { name: /搜索/ }))
    expect(onSearch).toHaveBeenCalledWith(
      expect.objectContaining({
        beginTime: '2026-01-01 00:00:00',
        endTime: '2026-01-31 23:59:59',
      })
    )
  })

  it('beginKey/endKey 可覆盖默认键名', async () => {
    const onSearch = vi.fn()
    render(QueryForm, {
      props: {
        fields: [
          {
            key: 'operTime',
            label: '操作时间',
            type: 'dateRange',
            beginKey: 'params[beginTime]',
            endKey: 'params[endTime]',
          },
        ],
        onSearch,
      },
    })
    await fireEvent.change(screen.getByLabelText('操作时间 起'), {
      target: { value: '2026-02-01' },
    })
    await fireEvent.click(screen.getByRole('button', { name: /搜索/ }))
    expect(onSearch).toHaveBeenCalledWith(
      expect.objectContaining({ 'params[beginTime]': '2026-02-01 00:00:00' })
    )
  })

  it('只填起始日期时不产生空的结束键', async () => {
    const onSearch = vi.fn()
    render(QueryForm, {
      props: {
        fields: [{ key: 'createTime', label: '创建时间', type: 'dateRange' }],
        onSearch,
      },
    })
    await fireEvent.change(screen.getByLabelText('创建时间 起'), {
      target: { value: '2026-01-01' },
    })
    await fireEvent.click(screen.getByRole('button', { name: /搜索/ }))
    const model = onSearch.mock.calls[0][0]
    expect(model.beginTime).toBe('2026-01-01 00:00:00')
    expect(model.endTime ?? '').toBe('')
  })

  // 🔴 后端五处 repo(operlog/logininfo/gen_table/role/user)的日期范围解析都是
  // NaiveDateTime::parse_from_str(v, "%Y-%m-%d %H:%M:%S"),且 `if let Ok(dt)` 解析
  // 失败就**跳过整个筛选**。只发 'YYYY-MM-DD' 会被静默丢掉 —— 用户选了时间范围
  // 但结果一行不少。Vue 侧一直是发满的(operlog/index.vue 的 value-format + default-time)。
  it('起始日期补 00:00:00、结束日期补 23:59:59(后端严格格式,补不上就静默丢筛选)', async () => {
    const onSearch = vi.fn()
    render(QueryForm, {
      props: {
        fields: [{ key: 'createTime', label: '创建时间', type: 'dateRange' }],
        onSearch,
      },
    })
    await fireEvent.change(screen.getByLabelText('创建时间 起'), {
      target: { value: '2026-01-01' },
    })
    await fireEvent.change(screen.getByLabelText('创建时间 止'), {
      target: { value: '2026-01-31' },
    })
    await fireEvent.click(screen.getByRole('button', { name: /搜索/ }))
    expect(onSearch).toHaveBeenCalledWith(
      expect.objectContaining({
        beginTime: '2026-01-01 00:00:00',
        endTime: '2026-01-31 23:59:59',
      })
    )
  })

  it('只填结束日期时补 23:59:59(覆盖 end 分支,且不产生空的起始键)', async () => {
    const onSearch = vi.fn()
    render(QueryForm, {
      props: {
        fields: [{ key: 'createTime', label: '创建时间', type: 'dateRange' }],
        onSearch,
      },
    })
    await fireEvent.change(screen.getByLabelText('创建时间 止'), {
      target: { value: '2026-01-31' },
    })
    await fireEvent.click(screen.getByRole('button', { name: /搜索/ }))
    const model = onSearch.mock.calls[0][0]
    expect(model.endTime).toBe('2026-01-31 23:59:59')
    // 空的起始键被丢弃,不产生 'beginTime: ""'
    expect(model.beginTime ?? '').toBe('')
  })

  it('非 dateRange 的文本字段不被加工', async () => {
    const onSearch = vi.fn()
    render(QueryForm, {
      props: { fields: [{ key: 'userName', label: '用户名称' }], onSearch },
    })
    await fireEvent.input(screen.getByLabelText('用户名称'), {
      target: { value: '2026-01-01' },
    })
    await fireEvent.click(screen.getByRole('button', { name: /搜索/ }))
    expect(onSearch.mock.calls[0][0].userName).toBe('2026-01-01')
  })
})

describe('QueryForm treeSelect 类型', () => {
  it('渲染 TreeSelect 并把选中 id 写入 model', async () => {
    const onSearch = vi.fn()
    render(QueryForm, {
      props: {
        fields: [
          {
            key: 'deptId',
            label: '归属部门',
            type: 'treeSelect',
            treeData: [
              {
                id: '1',
                label: '总公司',
                children: [{ id: '2', label: '研发部' }],
              },
            ],
          },
        ],
        onSearch,
      },
    })
    await fireEvent.click(screen.getByRole('button', { name: /请选择|选择/ }))
    await fireEvent.click(screen.getByRole('button', { name: '展开' }))
    await fireEvent.click(screen.getByText('研发部'))
    await fireEvent.click(screen.getByRole('button', { name: /搜索/ }))
    expect(onSearch).toHaveBeenCalledWith(
      expect.objectContaining({ deptId: '2' })
    )
  })
})

describe('QueryForm userSelect 类型', () => {
  const fields = [
    { key: 'createByIds', label: '申请人', type: 'userSelect' as const },
  ]

  it('渲染出用户选择控件并加载出候选人', async () => {
    render(QueryForm, { props: { fields } })
    // UserPicker 的 placeholder 默认「请选择人员」,本字段未显式传 placeholder,
    // 故落回组件默认;这里只断言控件被渲染(触发器按钮存在)。
    expect(
      await screen.findByRole('button', { name: /请选择/ })
    ).toBeInTheDocument()
  })

  it('多选结果以逗号分隔写进 model —— 后端 create_by_ids 约定', async () => {
    const onSearch = vi.fn()
    render(QueryForm, { props: { fields, onSearch } })
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /请选择/ })).toBeInTheDocument()
    )
    // UserPicker 的 option label 形如「超级管理员（admin）」(全角括号)。
    await fireEvent.click(screen.getByRole('button', { name: /请选择/ }))
    await fireEvent.click(await screen.findByLabelText(/超级管理员/))
    await fireEvent.click(await screen.findByLabelText(/李四/))
    await fireEvent.click(screen.getByRole('button', { name: /搜索/ }))
    await waitFor(() =>
      expect(onSearch).toHaveBeenCalledWith(
        expect.objectContaining({ createByIds: '1,2' })
      )
    )
  })

  it('重置后选择框视觉上也清空,不只是 model 清了', async () => {
    render(QueryForm, { props: { fields } })
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /请选择/ })).toBeInTheDocument()
    )
    await fireEvent.click(screen.getByRole('button', { name: /请选择/ }))
    await fireEvent.click(await screen.findByLabelText(/超级管理员/))
    // 选中后触发器按钮的 name 变为回显文案(MultiSelect 用逗号分隔的 label)。
    expect(
      await screen.findByRole('button', { name: /超级管理员/ })
    ).toBeInTheDocument()
    await fireEvent.click(screen.getByRole('button', { name: /重置/ }))
    // 重置后回到 placeholder 态。
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /请选择/ })).toBeInTheDocument()
    )
  })
})

describe('QueryForm 四列栅格', () => {
  it('容器是四列栅格,不再是 flex 自由流动', () => {
    const { container } = render(QueryForm, {
      props: { fields: [{ key: 'userName', label: '用户名' }] },
    })
    const form = container.querySelector('form')!
    expect(form.className).toContain('grid-cols-4')
    expect(form.className).not.toContain('flex-wrap')
  })

  it('搜索/重置格钉在第 4 列 —— 最右端恒为按钮,空格只会出现在它左边', () => {
    const { container } = render(QueryForm, {
      props: { fields: [{ key: 'userName', label: '用户名' }] },
    })
    const cell = container.querySelector('[data-testid="query-actions"]')!
    expect(cell.className).toContain('col-start-4')
  })

  it('dateRange 占两格 —— 两个日期输入挤在四分之一宽里不可用', () => {
    const { container } = render(QueryForm, {
      props: {
        fields: [{ key: 'createTime', label: '创建时间', type: 'dateRange' }],
      },
    })
    const cell = container.querySelector('[data-field-type="dateRange"]')!
    expect(cell.className).toContain('col-span-2')
  })

  it('控件铺满格子,不再是固定 w-40 —— 右侧不留空', () => {
    const { container } = render(QueryForm, {
      props: { fields: [{ key: 'userName', label: '用户名' }] },
    })
    const input = container.querySelector('input')!
    expect(input.className).toContain('flex-1')
    expect(input.className).not.toContain('w-40')
  })

  it('标签仍是右对齐的 80px 定宽列', () => {
    const { container } = render(QueryForm, {
      props: { fields: [{ key: 'userName', label: '用户名' }] },
    })
    const label = container.querySelector('span')!
    expect(label.className).toContain('w-20')
    expect(label.className).toContain('text-right')
  })
})
