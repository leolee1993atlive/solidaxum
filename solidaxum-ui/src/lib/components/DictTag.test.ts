import { cleanup, render, screen, waitFor } from '@testing-library/svelte'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import type { DictDataVo } from '$lib/api/system/dict.js'
import Fixture from './DictTag.fixture.svelte'
import { DICT_TAG_CLASS } from './dict-tag'
import { dictCache } from '$lib/hooks/use-dict'

const rows: DictDataVo[] = [
  { dictCode: '1', dictSort: 1, dictLabel: '正常', dictValue: '0', dictType: 'sys_normal_disable', listClass: 'primary', isDefault: 'Y' },
  { dictCode: '2', dictSort: 2, dictLabel: '停用', dictValue: '1', dictType: 'sys_normal_disable', listClass: 'danger', isDefault: 'N' },
]

beforeEach(() => dictCache.clear())
afterEach(cleanup)

describe('DictTag', () => {
  it('按字典值渲染标签文本', async () => {
    render(Fixture, { props: { dictType: 'sys_normal_disable', value: '1', rows } })
    await waitFor(() => expect(screen.getByText('停用')).toBeInTheDocument())
  })

  it('按 listClass 应用配色 class(而非落回 Badge 默认色)', async () => {
    render(Fixture, { props: { dictType: 'sys_normal_disable', value: '1', rows } })
    const el = await screen.findByText('停用')
    expect(el.className).toContain(DICT_TAG_CLASS.danger.split(' ')[1]) // bg-red-100
  })

  it('字典未命中时退化为纯文本原码,不渲染 Badge', async () => {
    render(Fixture, { props: { dictType: 'sys_normal_disable', value: '9', rows } })
    const el = await screen.findByText('9')
    expect(el.tagName.toLowerCase()).toBe('span')
    expect(el.className).toBe('')
  })

  it('value 为 undefined 时不炸', async () => {
    render(Fixture, { props: { dictType: 'sys_normal_disable', value: undefined, rows } })
    await waitFor(() => expect(screen.getByTestId('host')).toBeInTheDocument())
  })
})
