import { cleanup, render, screen } from '@testing-library/svelte'
import { afterEach, describe, expect, it } from 'vitest'
import Fixture from './HasPermi.fixture.svelte'

afterEach(cleanup)

describe('HasPermi', () => {
  it('拥有该权限时渲染子内容', () => {
    render(Fixture, { props: { perms: ['system:user:add'], perm: 'system:user:add' } })
    expect(screen.queryByTestId('guarded')).toBeInTheDocument()
  })

  it('无该权限时隐藏子内容', () => {
    render(Fixture, { props: { perms: ['system:user:edit'], perm: 'system:user:add' } })
    expect(screen.queryByTestId('guarded')).not.toBeInTheDocument()
  })

  it('super-admin *:*:* 命中任意权限', () => {
    render(Fixture, { props: { perms: ['*:*:*'], perm: 'system:user:add' } })
    expect(screen.queryByTestId('guarded')).toBeInTheDocument()
  })

  it('数组权限任一命中即渲染', () => {
    render(Fixture, {
      props: { perms: ['system:user:edit'], perm: ['system:user:add', 'system:user:edit'] },
    })
    expect(screen.queryByTestId('guarded')).toBeInTheDocument()
  })

  it('数组权限全不命中则隐藏', () => {
    render(Fixture, {
      props: { perms: [], perm: ['system:user:add', 'system:user:edit'] },
    })
    expect(screen.queryByTestId('guarded')).not.toBeInTheDocument()
  })
})
