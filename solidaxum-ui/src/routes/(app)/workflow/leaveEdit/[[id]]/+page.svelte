<script lang="ts">
  import { goto } from '$app/navigation'
  import { page } from '$app/state'
  import { toast } from 'svelte-sonner'

  import { leaveApi } from '$lib/api/workflow/leave.js'
  import type { LeaveVo } from '$lib/api/workflow/leave.js'
  import {
    LEAVE_TYPE_OPTIONS,
    FLOW_CODE_OPTIONS,
    leaveTypeLabel,
    buildLeaveSubmitBo,
  } from '$lib/api/workflow/leave-options.js'
  import ApprovalRecord from '$lib/components/process/ApprovalRecord.svelte'
  import ApprovalButton from '$lib/components/process/ApprovalButton.svelte'
  import Main from '$lib/components/layout/Main.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'

  type Mode = 'add' | 'update' | 'view' | 'approval'

  const id = page.params.id ?? ''
  const mode = (page.url.searchParams.get('mode') ?? 'add') as Mode
  const taskId = page.url.searchParams.get('taskId') ?? ''
  /** 返回路径:从待办/已办/抄送页进来时带上,默认回请假列表(向后兼容)。 */
  const from = page.url.searchParams.get('from') ?? '/demo/leave'
  const readonly = mode === 'view' || mode === 'approval'

  interface LeaveFormModel {
    leaveType: string
    /** 🔴 后端必填(dto.rs:885),漏发 → 新增恒 400。 */
    leaveDays: number
    remark: string
  }

  let form = $state<LeaveFormModel>({ leaveType: '1', leaveDays: 1, remark: '' })
  /** 只读展示用:后端推算出来的起止日期,新增时为空。 */
  let startDate = $state('')
  let endDate = $state('')
  let status = $state<string | undefined>(undefined)
  let loading = $state(mode !== 'add')
  let saving = $state(false)

  // 对齐 Vue leaveEdit.vue:26(仅 add 模式显示),默认 leave1。
  let flowCode = $state('leave1')
  const flowNote = $derived(FLOW_CODE_OPTIONS.find((o) => o.value === flowCode)?.note)

  $effect(() => {
    if (mode === 'add' || !id) return
    void (async () => {
      try {
        const resp = await leaveApi.getById(id)
        const vo = (resp.data ?? resp) as LeaveVo
        form = {
          leaveType: vo.leaveType ?? '1',
          leaveDays: vo.leaveDays ?? 1,
          remark: vo.remark ?? '',
        }
        startDate = vo.startDate ?? ''
        endDate = vo.endDate ?? ''
        status = vo.status
      } catch (e) {
        toast.error(e instanceof Error ? e.message : '加载失败')
      } finally {
        loading = false
      }
    })()
  })

  function back() {
    goto(from)
  }

  /** 前端最后一道防线:后端 validate 只会回一句英文 missing field,自己先拦更友好。 */
  function formIssues(): string[] {
    const issues: string[] = []
    if (!form.leaveType) issues.push('请假类型不能为空')
    if (!Number.isFinite(form.leaveDays) || form.leaveDays < 1) {
      issues.push('请假天数至少 1 天')
    }
    return issues
  }

  /** 暂存(不启动流程)。status 显式发 draft,对齐 Vue submitForm(status='draft')。 */
  async function save() {
    const issues = formIssues()
    if (issues.length > 0) {
      toast.error(issues[0])
      return
    }
    saving = true
    try {
      if (mode === 'add') {
        await leaveApi.create({
          leaveType: form.leaveType,
          leaveDays: form.leaveDays,
          remark: form.remark || undefined,
          status: 'draft',
        })
      } else {
        await leaveApi.update({
          id,
          leaveType: form.leaveType,
          leaveDays: form.leaveDays,
          remark: form.remark || undefined,
          status,
        })
      }
      toast.success('暂存成功')
      back()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : '保存失败')
    } finally {
      saving = false
    }
  }

  /**
   * 提交并启动流程。后端要 workflow:leave:add 权限(leave_handler.rs:175)。
   * 🔴 update 模式必须把 id 一起发出去,否则后端新建一条重复单据
   * (见 buildLeaveSubmitBo 的注释与 leave-options.test.ts)。
   */
  async function submitFlow() {
    const issues = formIssues()
    if (issues.length > 0) {
      toast.error(issues[0])
      return
    }
    saving = true
    try {
      await leaveApi.submitAndStart(
        buildLeaveSubmitBo({
          mode: mode === 'update' ? 'update' : 'add',
          id,
          leaveType: form.leaveType,
          leaveDays: form.leaveDays,
          remark: form.remark,
          flowCode,
        })
      )
      toast.success('提交成功')
      back()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : '提交失败')
    } finally {
      saving = false
    }
  }
</script>

<Main class="flex flex-1 flex-col gap-4">
  <Button variant="outline" onclick={back} class="w-fit">返回</Button>

  {#if loading}
    <div class="text-muted-foreground py-8 text-center">加载中...</div>
  {:else}
    <div class="grid max-w-xl gap-4 rounded-lg border p-4">
      {#if mode === 'add'}
        <div class="grid gap-2">
          <Label for="flowCode">流程定义</Label>
          <select id="flowCode" class="rounded border px-2 py-1" bind:value={flowCode}>
            {#each FLOW_CODE_OPTIONS as o (o.value)}
              <option value={o.value} disabled={o.support === 'unsupported'}>
                {o.label}{o.support === 'unsupported' ? '（本仓不支持）' : ''}
              </option>
            {/each}
          </select>
          {#if flowNote}
            <p class="text-xs text-warning-emphasis dark:text-warning-emphasis">⚠️ {flowNote}</p>
          {/if}
        </div>
      {/if}

      <div class="grid gap-2">
        <Label for="leaveType">请假类型</Label>
        <select
          id="leaveType"
          class="rounded border px-2 py-1"
          bind:value={form.leaveType}
          disabled={readonly}>
          {#each LEAVE_TYPE_OPTIONS as o (o.value)}
            <option value={o.value}>{o.label}</option>
          {/each}
        </select>
      </div>

      <div class="grid gap-2">
        <Label for="leaveDays">请假天数</Label>
        <Input
          id="leaveDays"
          type="number"
          min="1"
          bind:value={form.leaveDays}
          disabled={readonly} />
        <!-- 本仓契约与 Vue 相反 —— Vue 由日期算天数,
             本仓后端由天数推日期(leave_service.rs:66-78)。故天数是输入,日期只读。 -->
        <p class="text-muted-foreground text-xs">
          起止日期由服务端按「提交时间 + 天数」推算，不接受手工指定。
        </p>
      </div>

      {#if mode !== 'add'}
        <div class="grid gap-2 sm:grid-cols-2">
          <div class="grid gap-2">
            <Label>开始时间</Label>
            <Input value={startDate || '-'} disabled />
          </div>
          <div class="grid gap-2">
            <Label>结束时间</Label>
            <Input value={endDate || '-'} disabled />
          </div>
        </div>
      {/if}

      <div class="grid gap-2">
        <Label for="remark">请假原因</Label>
        <Textarea id="remark" bind:value={form.remark} rows={3} disabled={readonly} />
      </div>

      {#if !readonly}
        <div class="flex gap-2">
          <Button variant="outline" disabled={saving} onclick={save}>暂存</Button>
          <Button disabled={saving} onclick={submitFlow}>提交</Button>
        </div>
      {/if}
    </div>

    {#if mode === 'approval' && taskId}
      <div class="rounded-lg border p-4">
        <h3 class="mb-3 font-medium">审批操作</h3>
        <ApprovalButton {taskId} onDone={back} />
      </div>
    {/if}

    {#if readonly && id}
      <div class="rounded-lg border p-4">
        <h3 class="mb-3 font-medium">审批记录</h3>
        <ApprovalRecord businessId={id} />
      </div>
    {/if}
  {/if}
</Main>
