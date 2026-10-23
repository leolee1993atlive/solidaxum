import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

const ROUTES = join(process.cwd(), 'src', 'routes')

// 任务三页,护栏范围限定于此。
const TASK_PAGES = ['task/taskWaiting', 'task/taskFinish', 'task/taskCopyList']

function walk(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name)
    if (statSync(p).isDirectory()) walk(p, out)
    else if (p.endsWith('.svelte')) out.push(p)
  }
  return out
}

describe('applyCode 归零护栏', () => {
  // 后端 FlowTaskVo / FlowHisTaskVo(dto.rs) 都没有 apply_code。
  // 三个任务页曾把「业务编号」列绑在 FlowTaskVo.applyCode 上(不存在),恒显示 '-'。
  // 正确字段是 businessCode。VO 的 [k:string]:unknown 索引签名让 TS 拦不住，只能靠扫描测试兜底。
  //
  // ⚠️ 范围限定:只扫任务三页(task/taskWaiting|taskFinish|taskCopyList)的列绑定,
  // 不扫全站 —— 因为 LeaveVo.apply_code(dto.rs:918 / test_leave 实体)是真实存在的
  // 请假申请编号字段,demo/leave 页用它完全合法。myDocument 页的同名问题不在
  // 本护栏范围。
  it('任务三页的列不再把 accessorKey 绑到 applyCode', () => {
    const offenders = walk(ROUTES)
      .filter((f) => TASK_PAGES.some((p) => f.replaceAll('\\', '/').includes(p)))
      .filter((f) => /accessor(Key|Fn)\s*[:=]\s*\(?['"`]?r?\.?applyCode/.test(readFileSync(f, 'utf8')))
      .map((f) => f.replaceAll('\\', '/').split('/src/')[1])
    expect(offenders).toEqual([])
  })

  it('mytask.ts 的 FlowTaskVo/FlowHisTaskVo 不声明 applyCode 字段', () => {
    const src = readFileSync(join(process.cwd(), 'src', 'lib', 'api', 'workflow', 'mytask.ts'), 'utf8')
    // 排除注释行(说明性提及合理),只拦字段声明。
    const fieldDecl = src.match(/^\s*applyCode\??:\s*string/m)
    expect(fieldDecl).toBeNull()
  })
})

describe('任务三页跳转入口护栏', () => {
  // 三页的行操作曾是空 <span>(待办根本办不了)。
  const PAGES: Array<[string, string]> = [
    ['task/taskWaiting', 'buildFormUrl'],
    ['task/taskFinish', 'buildFormUrl'],
    ['task/taskCopyList', 'buildFormUrl'],
  ]

  it.each(PAGES)('%s 使用 %s 构造跳转 URL', (page, call) => {
    const hit = walk(ROUTES)
      .filter((f) => f.replaceAll('\\', '/').includes(page))
      .some((f) => readFileSync(f, 'utf8').includes(call))
    expect(hit).toBe(true)
  })

  it('任务三页不手工拼 leaveEdit 路径(必须走 buildFormUrl 吸收 /index 差异)', () => {
    // 范围限定 task 目录:demo/leave 列表页的跳转不在本护栏范围。
    const offenders = walk(ROUTES)
      .filter((f) => {
        const rel = f.replaceAll('\\', '/')
        return TASK_PAGES.some((p) => rel.includes(p))
      })
      .filter((f) => /['"`]\/workflow\/leaveEdit/.test(readFileSync(f, 'utf8')))
      .map((f) => f.replaceAll('\\', '/').split('/src/')[1])
    expect(offenders).toEqual([])
  })
})

describe('myDocument / allTaskWaiting 入口护栏', () => {
  // myDocument cellRenderer 曾是空 <span>(行操作全缺)。
  // allTaskWaiting 曾只调 pageByAllTaskWait(无已办 Tab)。
  // 这组护栏只防「有人后来把入口/端点删了或改回我的已办」,真正有价值的是
  // task-contract.test.ts 锁的 updateAssignee/urge/cancelProcessApply 方法契约。
  const ROUTES = join(process.cwd(), 'src', 'routes')
  const readPage = (rel: string) => readFileSync(join(ROUTES, rel, '+page.svelte'), 'utf8')

  it('myDocument 使用 buildFormUrl 与撤销端点 + 分类树', () => {
    const src = readPage('(app)/task/myDocument')
    expect(src).toContain('buildFormUrl')
    expect(src).toContain('cancelProcessApply')
    expect(src).toContain('TreePanel')
  })

  it('myDocument 删除/撤销有状态守卫(非无条件渲染)', () => {
    const src = readPage('(app)/task/myDocument')
    // Vue 守卫:编辑/删除 draft|cancel|back;撤销 waiting。护栏只确认守卫函数存在。
    expect(src).toContain('isEditable')
    expect(src).toContain('isCancelable')
  })

  it('allTaskWaiting 同时引用全部待办与全部已办端点', () => {
    const src = readPage('(app)/workflow/processMonitor/allTaskWaiting')
    expect(src).toContain('pageByAllTaskWait')
    // 不是 pageFinish/pageByTaskFinish(那是「我的已办」)
    expect(src).toContain('pageByAllTaskFinish')
  })

  it('allTaskWaiting 不再有多余「流程编码」筛选框(ATW4)且已删越权办理(ATW9)', () => {
    const src = readPage('(app)/workflow/processMonitor/allTaskWaiting')
    // queryFields 不含 flowCode 筛选项
    expect(src).not.toMatch(/key:\s*['"`]flowCode['"`]/)
    // 简易办理弹窗已删
    expect(src).not.toContain('completeTarget')
  })
})

