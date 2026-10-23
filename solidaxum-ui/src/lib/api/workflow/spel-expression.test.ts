import { describe, expect, it } from 'vitest'
import { buildViewSpel } from './spel-expression'

describe('buildViewSpel', () => {
  it('三者皆空 → 空串', () => {
    expect(buildViewSpel({ componentName: '', methodName: '', methodParams: '' })).toBe('')
    expect(buildViewSpel({})).toBe('')
  })

  it('只有单个参数(组件与方法都空) → ${参数},用 $ 不是 #', () => {
    expect(buildViewSpel({ methodParams: 'deptId' })).toBe('${deptId}')
  })

  it('只有多个参数时不走 ${} 分支,落到「请填写」提示', () => {
    expect(buildViewSpel({ methodParams: 'a,b' })).toBe('请填写组件名称和方法名')
  })

  it('缺组件或缺方法 → 把提示文案写进值里', () => {
    expect(buildViewSpel({ componentName: 'spelRuleComponent' })).toBe('请填写组件名称和方法名')
    expect(buildViewSpel({ methodName: 'selectDeptLeaderById' })).toBe('请填写组件名称和方法名')
  })

  it('组件+方法齐全、无参 → 括号仍在', () => {
    expect(buildViewSpel({ componentName: 'spelRuleComponent', methodName: 'selectLeader' })).toBe(
      '#{@spelRuleComponent.selectLeader()}'
    )
  })

  it('组件+方法+多参 → 每个参数前加 #,逗号分隔', () => {
    expect(
      buildViewSpel({ componentName: 'c', methodName: 'm', methodParams: 'deptId,userId' })
    ).toBe('#{@c.m(#deptId,#userId)}')
  })

  it('参数串逐个 trim 并丢弃空串', () => {
    expect(buildViewSpel({ componentName: 'c', methodName: 'm', methodParams: ' a , , b ,' })).toBe(
      '#{@c.m(#a,#b)}'
    )
  })

  it('组件/方法名两侧空白被 trim', () => {
    expect(buildViewSpel({ componentName: '  c  ', methodName: '  m  ' })).toBe('#{@c.m()}')
  })

  it('undefined 与空串等价', () => {
    expect(buildViewSpel({ componentName: undefined, methodName: undefined, methodParams: undefined })).toBe('')
  })
})

describe('isSubmittableSpel', () => {
  it('空串与提示文案都不可提交 —— 后端 view_spel 有 min=1 校验且提示文案不是表达式', async () => {
    const { isSubmittableSpel } = await import('./spel-expression')
    expect(isSubmittableSpel('')).toBe(false)
    expect(isSubmittableSpel('请填写组件名称和方法名')).toBe(false)
    expect(isSubmittableSpel('#{@c.m()}')).toBe(true)
    expect(isSubmittableSpel('${deptId}')).toBe(true)
  })
})
