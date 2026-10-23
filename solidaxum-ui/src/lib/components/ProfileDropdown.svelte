<script lang="ts">
  import { goto } from '$app/navigation'
  import {
    Avatar,
    AvatarFallback,
    AvatarImage,
  } from '$lib/components/ui/avatar/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu/index.js'
  import { useAuth } from '$lib/context/auth.svelte.js'
  import {
    Building2,
    KeyRound,
    LogOut,
    MonitorSmartphone,
    User,
  } from '@lucide/svelte'
  import { summarizeIdentity } from './profile-identity.js'

  const auth = useAuth()
  const identity = $derived(summarizeIdentity(auth.user))
</script>

<DropdownMenu.Root>
  <DropdownMenu.Trigger
    class="focus-visible:ring-ring rounded-full focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none"
    aria-label="用户菜单"
  >
    <Avatar class="h-9 w-9 border">
      <AvatarImage src={auth.user?.avatarUrl} alt={identity.displayName} />
      <AvatarFallback>{identity.initial}</AvatarFallback>
    </Avatar>
  </DropdownMenu.Trigger>

  <DropdownMenu.Content class="w-56" align="end" sideOffset={4}>
    <!-- 身份区:纯展示,不可点击 -->
    <div class="flex items-start gap-2 px-2 py-2">
      <Avatar class="h-9 w-9 shrink-0 border">
        <AvatarImage src={auth.user?.avatarUrl} alt={identity.displayName} />
        <AvatarFallback>{identity.initial}</AvatarFallback>
      </Avatar>
      <div class="min-w-0 flex-1">
        <div class="truncate text-sm font-medium">{identity.displayName}</div>
        {#if auth.user?.userName}
          <div class="text-muted-foreground truncate text-xs">
            @{auth.user.userName}
          </div>
        {/if}
        {#if identity.shownRoles.length > 0}
          <div class="mt-1.5 flex flex-wrap items-center gap-1">
            {#each identity.shownRoles as role (role.roleId)}
              <Badge
                variant="secondary"
                class="px-1.5 py-0 text-[10px] font-normal"
              >
                {role.roleName}
              </Badge>
            {/each}
            {#if identity.overflowCount > 0}
              <span class="text-muted-foreground text-[10px]">
                +{identity.overflowCount}
              </span>
            {/if}
          </div>
        {/if}
        {#if identity.deptName}
          <div
            class="text-muted-foreground mt-1 flex items-center gap-1 text-xs"
          >
            <Building2 class="h-3 w-3 shrink-0" />
            <span class="truncate">{identity.deptName}</span>
          </div>
        {/if}
      </div>
    </div>

    <DropdownMenu.Separator />

    <DropdownMenu.Item onclick={() => goto('/user/profile')}>
      <User class="mr-2 h-4 w-4" />
      个人中心
    </DropdownMenu.Item>
    <DropdownMenu.Item onclick={() => goto('/user/profile?tab=resetPwd')}>
      <KeyRound class="mr-2 h-4 w-4" />
      修改密码
    </DropdownMenu.Item>
    <DropdownMenu.Item onclick={() => goto('/user/profile?tab=onlineDevice')}>
      <MonitorSmartphone class="mr-2 h-4 w-4" />
      在线设备
    </DropdownMenu.Item>

    <DropdownMenu.Separator />

    <DropdownMenu.Item
      variant="destructive"
      onclick={async () => {
        await auth.logout()
        goto('/')
      }}
    >
      <LogOut class="mr-2 h-4 w-4" />
      退出登录
    </DropdownMenu.Item>
  </DropdownMenu.Content>
</DropdownMenu.Root>
