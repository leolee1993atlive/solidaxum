import { parseListeners } from '$lib/flow/node-fields.js'
import { cleanup, render, screen } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'
import ListenerRows from './ListenerRows.svelte'

vi.mock('$lib/api/workflow/designer.js', () => ({
  designerApi: {
    listenerList: vi.fn().mockResolvedValue({ code: 200, msg: 'ok', data: [] }),
  },
}))

afterEach(cleanup)

describe('ListenerRows', () => {
  it('存量拼串带尾分隔符(解析出全空行)时照常渲染,不陷入 effect 死循环', async () => {
    // 老设计器可能存出 "start," / "@@" 这类尾分隔符,解析后末尾是一行全空
    const rows = parseListeners('start,', 'com.x.LeaveListener')
    expect(rows).toEqual([
      { listenerType: 'start', listenerPath: 'com.x.LeaveListener' },
      { listenerType: '', listenerPath: '' },
    ])
    render(ListenerRows, { props: { rows, onChange: () => {} } })
    expect(screen.getAllByLabelText('监听器类型')).toHaveLength(2)
  })
})
