import { describe, it, expect, beforeEach, vi } from 'vitest'
import { dictCache, fetchDict, resolveDictCssClass, resolveDictLabel } from './use-dict'
import type { DictDataVo } from '$lib/api/system/dict.js'

beforeEach(() => {
  dictCache.clear()
  vi.restoreAllMocks()
})

describe('fetchDict caching', () => {
  it('caches by dictType; second call does not refetch', async () => {
    const fake: DictDataVo[] = [
      { dictCode: '1', dictSort: 0, dictLabel: '男', dictValue: '0', dictType: 'sys_user_gender', isDefault: 'Y' },
    ]
    const spy = vi.fn().mockResolvedValue(fake)
    const a = await fetchDict('sys_user_gender', spy)
    const b = await fetchDict('sys_user_gender', spy)
    expect(a).toEqual(fake)
    expect(b).toEqual(fake)
    expect(spy).toHaveBeenCalledTimes(1)
  })
})

describe('resolveDictLabel', () => {
  it('returns label for matching value', () => {
    const list: DictDataVo[] = [
      { dictCode: '1', dictSort: 0, dictLabel: '男', dictValue: '0', dictType: 'x', isDefault: 'Y' },
      { dictCode: '2', dictSort: 1, dictLabel: '女', dictValue: '1', dictType: 'x', isDefault: 'N' },
    ]
    expect(resolveDictLabel(list, '1')).toBe('女')
  })
  it('returns the raw value when not found', () => {
    expect(resolveDictLabel([], 'zzz')).toBe('zzz')
  })
})

describe('resolveDictCssClass', () => {
  const list: DictDataVo[] = [
    { dictCode: '1', dictSort: 0, dictLabel: '密码认证', dictValue: 'password', dictType: 'sys_grant_type', cssClass: 'el-check-tag', listClass: 'default', isDefault: 'N' },
    { dictCode: '2', dictSort: 1, dictLabel: '短信认证', dictValue: 'sms', dictType: 'sys_grant_type', isDefault: 'N' },
  ]
  it('命中时返回 cssClass', () => {
    expect(resolveDictCssClass(list, 'password')).toBe('el-check-tag')
  })
  it('命中但无 cssClass 时返回空串', () => {
    expect(resolveDictCssClass(list, 'sms')).toBe('')
  })
  it('未命中返回空串', () => {
    expect(resolveDictCssClass(list, 'zzz')).toBe('')
  })
})
