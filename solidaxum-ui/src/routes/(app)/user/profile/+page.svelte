<script lang="ts">
  import { goto } from '$app/navigation'
  import { page } from '$app/state'
  import { profileApi } from '$lib/api/system/profile.js'
  import type { ProfileVo } from '$lib/api/system/profile.js'
  import Main from '$lib/components/layout/Main.svelte'
  import AvatarUpload from '$lib/components/profile/AvatarUpload.svelte'
  import OnlineDeviceList from '$lib/components/profile/OnlineDeviceList.svelte'
  import ResetPwdForm from '$lib/components/profile/ResetPwdForm.svelte'
  import ThirdPartyList from '$lib/components/profile/ThirdPartyList.svelte'
  import UserInfoForm from '$lib/components/profile/UserInfoForm.svelte'
  import { resolveProfileTab } from '$lib/components/profile/tabs.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import {
    Building2,
    CalendarDays,
    Mail,
    Phone,
    User,
    UsersRound,
    BriefcaseBusiness,
  } from '@lucide/svelte'
  import { toast } from 'svelte-sonner'

  let profile = $state<ProfileVo | null>(null)
  let loading = $state(true)

  /** tab 以 URL 为单一真相,支持从头像下拉深链直达。 */
  const activeTab = $derived(
    resolveProfileTab(page.url.searchParams.get('tab'))
  )

  function selectTab(value: string) {
    const url = new URL(page.url)
    url.searchParams.set('tab', value)
    void goto(url, { noScroll: true, keepFocus: true })
  }

  async function reload() {
    try {
      const resp = await profileApi.get()
      profile = resp.data ?? null
    } catch (e) {
      toast.error(e instanceof Error ? e.message : '加载个人信息失败')
    } finally {
      loading = false
    }
  }

  $effect(() => {
    void reload()
  })
</script>

<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  {#if loading}
    <div class="text-muted-foreground py-8 text-center">加载中...</div>
  {:else if profile}
    <div class="grid gap-4 lg:grid-cols-[320px_1fr]">
      <!-- 左:个人信息卡 -->
      <div data-slot="content-card" class="bg-card rounded-lg border p-4">
        <h3 class="mb-3 font-medium">个人信息</h3>
        <AvatarUpload avatarUrl={profile.user.avatarUrl} onUploaded={reload} />
        <dl class="grid gap-2 text-sm">
          <div class="flex items-center justify-between border-b py-1.5">
            <dt class="text-muted-foreground flex items-center gap-1.5">
              <User class="h-3.5 w-3.5" />用户名称
            </dt>
            <dd class="truncate">{profile.user.userName}</dd>
          </div>
          <div class="flex items-center justify-between border-b py-1.5">
            <dt class="text-muted-foreground flex items-center gap-1.5">
              <Phone class="h-3.5 w-3.5" />手机号码
            </dt>
            <dd class="truncate">{profile.user.phoneNumber || '-'}</dd>
          </div>
          <div class="flex items-center justify-between border-b py-1.5">
            <dt class="text-muted-foreground flex items-center gap-1.5">
              <Mail class="h-3.5 w-3.5" />用户邮箱
            </dt>
            <dd class="truncate">{profile.user.email || '-'}</dd>
          </div>
          <div class="flex items-center justify-between border-b py-1.5">
            <dt class="text-muted-foreground flex items-center gap-1.5">
              <Building2 class="h-3.5 w-3.5" />所属部门
            </dt>
            <dd class="truncate">{profile.user.deptName || '-'}</dd>
          </div>
          <div class="flex items-center justify-between border-b py-1.5">
            <dt class="text-muted-foreground flex items-center gap-1.5">
              <UsersRound class="h-3.5 w-3.5" />所属角色
            </dt>
            <dd class="truncate">{profile.roleGroup || '-'}</dd>
          </div>
          <div class="flex items-center justify-between border-b py-1.5">
            <dt class="text-muted-foreground flex items-center gap-1.5">
              <BriefcaseBusiness class="h-3.5 w-3.5" />所属岗位
            </dt>
            <dd class="truncate">{profile.postGroup || '-'}</dd>
          </div>
          <div class="flex items-center justify-between border-b py-1.5">
            <dt class="text-muted-foreground flex items-center gap-1.5">
              <CalendarDays class="h-3.5 w-3.5" />创建日期
            </dt>
            <dd class="truncate">{profile.user.createTime || '-'}</dd>
          </div>
        </dl>
      </div>

      <!-- 右:个人设置 -->
      <div data-slot="content-card" class="bg-card rounded-lg border p-4">
        <div class="mb-3">
          <h3 class="font-medium">个人设置</h3>
          <p class="text-muted-foreground text-xs">
            维护个人资料、密码、第三方应用和在线设备。
          </p>
        </div>
        <Tabs.Root value={activeTab} onValueChange={selectTab}>
          <Tabs.List>
            <Tabs.Trigger value="userinfo">基本资料</Tabs.Trigger>
            <Tabs.Trigger value="resetPwd">修改密码</Tabs.Trigger>
            <Tabs.Trigger value="thirdParty">第三方应用</Tabs.Trigger>
            <Tabs.Trigger value="onlineDevice">在线设备</Tabs.Trigger>
          </Tabs.List>
          <Tabs.Content value="userinfo">
            <UserInfoForm user={profile.user} onSaved={reload} />
          </Tabs.Content>
          <Tabs.Content value="resetPwd">
            <ResetPwdForm />
          </Tabs.Content>
          <Tabs.Content value="thirdParty">
            <ThirdPartyList />
          </Tabs.Content>
          <Tabs.Content value="onlineDevice">
            <OnlineDeviceList />
          </Tabs.Content>
        </Tabs.Root>
      </div>
    </div>
  {/if}
</Main>
