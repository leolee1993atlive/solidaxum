<script lang="ts">
  import { toast } from 'svelte-sonner'

  import { passwordIssues, profileApi } from '$lib/api/system/profile.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'

  let oldPassword = $state('')
  let newPassword = $state('')
  let confirmPassword = $state('')
  let saving = $state(false)

  async function submit() {
    const issues = passwordIssues({ oldPassword, newPassword, confirmPassword })
    if (issues.length > 0) {
      toast.error(issues[0])
      return
    }
    saving = true
    try {
      await profileApi.updatePwd(oldPassword, newPassword)
      toast.success('修改成功')
      oldPassword = ''
      newPassword = ''
      confirmPassword = ''
    } catch (e) {
      toast.error(e instanceof Error ? e.message : '修改失败')
    } finally {
      saving = false
    }
  }
</script>

<div class="grid max-w-md gap-4 pt-4">
  <div class="grid gap-2">
    <Label>旧密码</Label>
    <Input type="password" bind:value={oldPassword} />
  </div>
  <div class="grid gap-2">
    <Label>新密码</Label>
    <Input type="password" bind:value={newPassword} />
  </div>
  <div class="grid gap-2">
    <Label>确认密码</Label>
    <Input type="password" bind:value={confirmPassword} />
  </div>
  <div><Button disabled={saving} onclick={submit}>保存</Button></div>
</div>
