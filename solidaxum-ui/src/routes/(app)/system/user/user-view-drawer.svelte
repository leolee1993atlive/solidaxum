<script lang="ts">
  import * as Sheet from '$lib/components/ui/sheet/index.js'
  import DictTag from '$lib/components/DictTag.svelte'
  import { userApi } from '$lib/api/system/user.js'
  import type { SysUserInfoVo } from '$lib/api/system/user.js'

  let { open = $bindable(false), userId }: { open?: boolean; userId?: string } = $props()

  let loading = $state(false)
  let info = $state<SysUserInfoVo | null>(null)

  // 打开且 userId 变了才拉;关闭时清空,避免下次打开先闪上一个人的资料。
  $effect(() => {
    if (!open || !userId) {
      if (!open) info = null
      return
    }
    loading = true
    userApi
      .getById(userId)
      .then((res) => {
        info = res.data ?? null
      })
      .catch(() => {
        // apiClient 已 toast
      })
      .finally(() => {
        loading = false
      })
  })

  const roleNames = $derived(
    (info?.roles ?? [])
      .filter((r) => (info?.roleIds ?? []).includes(r.roleId))
      .map((r) => r.roleName)
      .join('、')
  )
  const postNames = $derived(
    (info?.posts ?? [])
      .filter((p) => (info?.postIds ?? []).includes(p.postId))
      .map((p) => p.postName)
      .join('、')
  )
</script>

<Sheet.Root bind:open>
  <Sheet.Content side="right" class="w-full sm:max-w-[560px]">
    <Sheet.Header><Sheet.Title>用户信息详情</Sheet.Title></Sheet.Header>
    {#if loading}
      <p class="p-4 text-sm text-muted-foreground">加载中…</p>
    {:else if info}
      {@const u = info.user}
      <dl class="grid grid-cols-2 gap-x-4 gap-y-3 p-4 text-sm">
        <div><dt class="text-muted-foreground">用户名称</dt><dd>{u.nickName}</dd></div>
        <div><dt class="text-muted-foreground">归属部门</dt><dd>{u.deptName ?? '-'}</dd></div>
        <div><dt class="text-muted-foreground">手机号码</dt><dd>{u.phoneNumber ?? '-'}</dd></div>
        <div><dt class="text-muted-foreground">邮箱</dt><dd>{u.email ?? '-'}</dd></div>
        <div><dt class="text-muted-foreground">登录账号</dt><dd>{u.userName}</dd></div>
        <div>
          <dt class="text-muted-foreground">用户状态</dt>
          <dd><DictTag dictType="sys_normal_disable" value={u.status} /></dd>
        </div>
        <div><dt class="text-muted-foreground">岗位</dt><dd>{postNames || '-'}</dd></div>
        <div>
          <dt class="text-muted-foreground">用户性别</dt>
          <dd><DictTag dictType="sys_user_gender" value={u.gender} /></dd>
        </div>
        <div class="col-span-2">
          <dt class="text-muted-foreground">角色</dt><dd>{roleNames || '-'}</dd>
        </div>
        <div><dt class="text-muted-foreground">创建时间</dt><dd>{u.createTime ?? '-'}</dd></div>
        <div><dt class="text-muted-foreground">更新时间</dt><dd>{u.updateTime ?? '-'}</dd></div>
        <div><dt class="text-muted-foreground">最后登录IP</dt><dd>{u.loginIp ?? '-'}</dd></div>
        <div><dt class="text-muted-foreground">最后登录时间</dt><dd>{u.loginDate ?? '-'}</dd></div>
        <div class="col-span-2"><dt class="text-muted-foreground">备注</dt><dd>{u.remark ?? '-'}</dd></div>
      </dl>
    {/if}
  </Sheet.Content>
</Sheet.Root>
