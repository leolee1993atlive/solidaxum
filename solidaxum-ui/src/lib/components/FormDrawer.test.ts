import { cleanup, render, screen } from '@testing-library/svelte'
import { describe, expect, it, afterEach } from 'vitest'
import FormDrawer from './FormDrawer.svelte'

afterEach(cleanup)

describe('FormDrawer 宽度', () => {
  // 默认宽度对齐 MeasureFormDrawer 的 xl(576px,单列表单);
  // 更宽场景用 size 档位(2xl/4xl),非常规宽度才用 class 覆盖。
  it('不传 size 时用默认档位 xl(适合单列表单)', () => {
    render(FormDrawer, {
      props: { open: true, onOpenChange: () => {}, title: '新增角色' },
    })
    expect(screen.getByRole('dialog').className).toContain('sm:max-w-xl')
  })

  it('size="2xl" 映射双列栅格档位', () => {
    render(FormDrawer, {
      props: {
        open: true,
        onOpenChange: () => {},
        title: '事件类型配置',
        size: '2xl',
      },
    })
    expect(screen.getByRole('dialog').className).toContain('sm:max-w-2xl')
  })

  it('size="4xl" 映射详情档位', () => {
    render(FormDrawer, {
      props: {
        open: true,
        onOpenChange: () => {},
        title: '审批记录',
        size: '4xl',
      },
    })
    expect(screen.getByRole('dialog').className).toContain('sm:max-w-4xl')
  })

  it('class 仍是 size 之外的逃生口(分配权限抽屉)', () => {
    render(FormDrawer, {
      props: {
        open: true,
        onOpenChange: () => {},
        title: '分配权限',
        class: 'sm:max-w-[760px]',
      },
    })
    expect(screen.getByRole('dialog').className).toContain('sm:max-w-[760px]')
  })
})

describe('FormDrawer readonly 变体', () => {
  it('默认(编辑态)渲染 取消 + 保存', () => {
    render(FormDrawer, {
      props: { open: true, onOpenChange: () => {}, title: '新增' },
    })
    expect(screen.getByRole('button', { name: '取消' })).toBeTruthy()
    expect(screen.getByRole('button', { name: '保存' })).toBeTruthy()
  })

  it('readonly 隐藏保存钮,取消变「关闭」', () => {
    render(FormDrawer, {
      props: {
        open: true,
        onOpenChange: () => {},
        title: '事件详情',
        readonly: true,
      },
    })
    expect(screen.getByRole('button', { name: '关闭' })).toBeTruthy()
    expect(screen.queryByRole('button', { name: '保存' })).toBeNull()
  })
})
