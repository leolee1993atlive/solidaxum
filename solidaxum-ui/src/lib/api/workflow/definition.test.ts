import { describe, expect, it } from 'vitest'
import { activeUrl, buildDefinitionForm, toDefinitionBody } from './definition.js'

describe('activeUrl', () => {
  // 后端 definition_handler.rs:119-121 的 ActiveQuery.active 为必填 bool,
  // 缺该查询参数即 400。deserialize_bool_str 接受 "true"/"1"/"false"/"0"。
  it('激活时带 active=true', () => {
    expect(activeUrl('7', true)).toBe('/workflow/definition/active/7?active=true')
  })
  it('挂起时带 active=false', () => {
    expect(activeUrl('7', false)).toBe('/workflow/definition/active/7?active=false')
  })
  it('URL 始终包含 active 查询参数', () => {
    expect(activeUrl('1', true)).toContain('?active=')
    expect(activeUrl('1', false)).toContain('?active=')
  })
})

describe('新增/编辑流程定义的表单往返', () => {
  it('toDefinitionBody 把 autoPass 序列化进 ext', () => {
    const body = toDefinitionBody(
      {
        id: '',
        flowCode: 'leave1',
        flowName: '请假',
        category: '100',
        modelValue: 'CLASSICS',
        formCustom: 'N',
        formPath: '',
        ext: '',
      },
      true
    )
    expect(body.ext).toBe('{"autoPass":true}')
    expect(body.flowCode).toBe('leave1')
    // 新增时不带 id,避免后端当成编辑
    expect('id' in body).toBe(false)
  })

  it('toDefinitionBody 在有 id 时保留 id(编辑)', () => {
    const body = toDefinitionBody(
      {
        id: '1761300000000000001',
        flowCode: 'leave1',
        flowName: '请假',
        category: '100',
        modelValue: 'CLASSICS',
        formCustom: 'N',
        formPath: '',
        ext: '',
      },
      false
    )
    expect(body.id).toBe('1761300000000000001')
    expect(body.ext).toBe('{"autoPass":false}')
  })

  it('buildDefinitionForm 解析 ext 还原 autoPass', () => {
    const { form, autoPass } = buildDefinitionForm({
      id: '7',
      flowCode: 'leave1',
      flowName: '请假',
      category: '100',
      modelValue: 'MIMIC',
      formCustom: 'N',
      formPath: '/leave',
      ext: '{"autoPass":true}',
    })
    expect(autoPass).toBe(true)
    expect(form.modelValue).toBe('MIMIC')
    expect(form.formPath).toBe('/leave')
  })

  it('buildDefinitionForm 对空/坏 ext 不抛异常,autoPass 归 false', () => {
    expect(buildDefinitionForm({ id: '7', ext: '' }).autoPass).toBe(false)
    expect(buildDefinitionForm({ id: '7', ext: 'not-json' }).autoPass).toBe(false)
    expect(buildDefinitionForm({ id: '7' }).autoPass).toBe(false)
  })
})
