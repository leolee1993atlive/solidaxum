<script lang="ts">
  import { profileApi } from '$lib/api/system/profile.js'
  import type { ProfileUserVo } from '$lib/api/system/profile.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import * as RadioGroup from '$lib/components/ui/radio-group/index.js'
  import { toast } from 'svelte-sonner'

  let { user, onSaved }: { user: ProfileUserVo; onSaved: () => void } = $props()

  let nickName = $state(user.nickName ?? '')
  let phoneNumber = $state(user.phoneNumber ?? '')
  let email = $state(user.email ?? '')
  let gender = $state(user.gender ?? '0')
  let saving = $state(false)

  // 校验文案逐条抄自 Vue userInfo.vue:41-60
  const EMAIL_RE = /^[\w.-]+@[\w-]+(\.[\w-]+)+$/
  const PHONE_RE = /^1[3-9]\d{9}$/

  async function submit() {
    if (!nickName.trim()) return toast.error('用户昵称不能为空')
    if (!email.trim()) return toast.error('邮箱地址不能为空')
    if (!EMAIL_RE.test(email)) return toast.error('请输入正确的邮箱地址')
    if (!phoneNumber.trim()) return toast.error('手机号码不能为空')
    if (!PHONE_RE.test(phoneNumber)) return toast.error('请输入正确的手机号码')

    saving = true
    try {
      await profileApi.update({ nickName, phoneNumber, email, gender })
      toast.success('修改成功')
      onSaved()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : '修改失败')
    } finally {
      saving = false
    }
  }
</script>

<div class="grid max-w-md gap-4 pt-4">
  <div class="grid gap-2">
    <Label>用户昵称</Label>
    <Input bind:value={nickName} maxlength={30} />
  </div>
  <div class="grid gap-2">
    <Label>手机号码</Label>
    <Input bind:value={phoneNumber} maxlength={11} />
  </div>
  <div class="grid gap-2">
    <Label>邮箱</Label>
    <Input bind:value={email} maxlength={50} />
  </div>
  <div class="grid gap-2">
    <Label>性别</Label>
    <RadioGroup.Root bind:value={gender} class="flex gap-6">
      <div class="flex items-center gap-2">
        <RadioGroup.Item value="0" id="gender-male" />
        <Label for="gender-male" class="font-normal">男</Label>
      </div>
      <div class="flex items-center gap-2">
        <RadioGroup.Item value="1" id="gender-female" />
        <Label for="gender-female" class="font-normal">女</Label>
      </div>
    </RadioGroup.Root>
  </div>
  <div class="flex gap-2">
    <Button disabled={saving} onclick={submit}>保存</Button>
  </div>
</div>
