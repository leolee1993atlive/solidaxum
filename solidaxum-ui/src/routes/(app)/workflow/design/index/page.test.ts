import { designerApi } from '$lib/api/workflow/designer.js'
import leave6Json from '$lib/flow/fixtures/leave6.json'
import { cleanup, render, screen, waitFor } from '@testing-library/svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import Page from './+page.svelte'

vi.mock('$app/state', () => ({
  page: {
    url: new URL(
      'http://localhost/workflow/design/index?definitionId=1&disabled=false&activeName=1'
    ),
  },
}))
vi.mock('$app/navigation', () => ({ goto: vi.fn(() => Promise.resolve()) }))
vi.mock('$lib/api/workflow/designer.js', () => ({
  designerApi: {
    queryDef: vi.fn(),
    saveJson: vi.fn(),
    listenerList: vi.fn(),
    handlerFeedback: vi.fn(),
  },
}))

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(designerApi.queryDef).mockResolvedValue({
    code: 200,
    msg: 'ok',
    data: leave6Json as never,
  })
})
afterEach(cleanup)

describe('流程设计页外壳', () => {
  // 旧 iframe 页复制来的裸 div 直接透出内容区渐变底;设计器要和其他页一样
  // 坐在 <Main> 里的白卡(data-slot=content-card + bg-card)上。
  it('设计器整体放在 content-card 白卡里', async () => {
    const { container } = render(Page)
    const card = container.querySelector('[data-slot="content-card"]')
    expect(card, '缺少 content-card 白卡').not.toBeNull()
    expect(card).toHaveClass('bg-card', 'rounded-lg')
    expect(container.querySelector('main')?.contains(card)).toBe(true)
    await waitFor(() =>
      expect(screen.getByRole('button', { name: '适应画布' })).toBeTruthy()
    )
    expect(
      card!.contains(screen.getByRole('button', { name: '适应画布' }))
    ).toBe(true)
  })
})
