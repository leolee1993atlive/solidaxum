<script lang="ts">
  import { toast } from 'svelte-sonner'
  import { goto } from '$app/navigation'
  import { page } from '$app/state'
  import { onMount } from 'svelte'

  import { userApi } from '$lib/api/system/user.js'
  import type { AuthRoleVo } from '$lib/api/system/user.js'
  import Main from '$lib/components/layout/Main.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import DictTag from '$lib/components/DictTag.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Checkbox } from '$lib/components/ui/checkbox/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import * as Table from '$lib/components/ui/table/index.js'

  const userId = page.params.userId!

  type RoleRow = AuthRoleVo['roles'][number]

  let loading = $state(true)
  let userInfo = $state<AuthRoleVo['user'] | null>(null)
  let roles = $state<RoleRow[]>([])
  /** 选中的 roleId 集合。初始值来自后端算好的 flag。 */
  let checked = $state(new Set<string>())
  let saving = $state(false)

  onMount(async () => {
    try {
      const res = await userApi.getAuthRole(userId)
      userInfo = res.data?.user ?? null
      roles = res.data?.roles ?? []
      checked = new Set(roles.filter((r) => r.flag).map((r) => r.roleId))
    } catch {
      // apiClient 已 toast
    } finally {
      loading = false
    }
  })

  /** 对齐 Vue checkSelectable:停用的角色不可勾选。 */
  function selectable(row: RoleRow): boolean {
    return row.status === '0'
  }

  function toggle(row: RoleRow) {
    if (!selectable(row)) return
    const next = new Set(checked)
    if (next.has(row.roleId)) next.delete(row.roleId)
    else next.add(row.roleId)
    checked = next
  }

  async function handleSubmit() {
    saving = true
    try {
      // 后端 AuthRoleQuery 走 **Query 参数**(user_handler.rs:486-490),roleIds 是逗号串。
      await userApi.updateAuthRole(userId, [...checked])
      toast.success('授权成功')
      goto('/system/user')
    } catch {
      // apiClient 已 toast
    } finally {
      saving = false
    }
  }
</script>

<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  <div class="flex gap-2">
    <HasPermi perm="system:user:edit">
      <Button disabled={saving || loading} onclick={handleSubmit}>
        {saving ? '提交中…' : '提交'}
      </Button>
    </HasPermi>
    <Button variant="outline" onclick={() => goto('/system/user')}>返回</Button>
  </div>

  <div class="grid gap-4 sm:grid-cols-2">
    <div class="flex flex-col gap-2">
      <Label for="nickName">用户昵称</Label>
      <Input id="nickName" value={userInfo?.nickName ?? ''} disabled />
    </div>
    <div class="flex flex-col gap-2">
      <Label for="userName">登录账号</Label>
      <Input id="userName" value={userInfo?.userName ?? ''} disabled />
    </div>
  </div>

  <div class="rounded-md border">
    <Table.Root>
      <Table.Header>
        <Table.Row>
          <Table.Head class="w-12"></Table.Head>
          <Table.Head>角色名称</Table.Head>
          <Table.Head>权限字符</Table.Head>
          <Table.Head>状态</Table.Head>
          <Table.Head>创建时间</Table.Head>
        </Table.Row>
      </Table.Header>
      <Table.Body>
        {#if loading}
          <Table.Row><Table.Cell colspan={5}>加载中…</Table.Cell></Table.Row>
        {:else if roles.length === 0}
          <Table.Row><Table.Cell colspan={5}>暂无角色数据。</Table.Cell></Table.Row>
        {:else}
          {#each roles as r (r.roleId)}
            <Table.Row>
              <Table.Cell>
                <Checkbox
                  checked={checked.has(r.roleId)}
                  disabled={!selectable(r)}
                  aria-label={`选择角色 ${r.roleName}`}
                  onCheckedChange={() => toggle(r)} />
              </Table.Cell>
              <Table.Cell>{r.roleName}</Table.Cell>
              <Table.Cell>{r.roleKey}</Table.Cell>
              <Table.Cell><DictTag dictType="sys_normal_disable" value={r.status} /></Table.Cell>
              <Table.Cell>{r.createTime ?? '-'}</Table.Cell>
            </Table.Row>
          {/each}
        {/if}
      </Table.Body>
    </Table.Root>
  </div>
</Main>
