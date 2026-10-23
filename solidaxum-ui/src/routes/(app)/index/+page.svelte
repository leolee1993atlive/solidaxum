<script lang="ts">
  import * as Card from '$lib/components/ui/card/index.js'
  import { useAuth } from '$lib/context/auth.svelte.js'
  import { APP_NAME } from '$lib/components/layout/data/sidebar-data.js'
  import { Activity, Database, Shield, Users } from '@lucide/svelte'

  const auth = useAuth()

  // 占位统计卡片(后续可接真实数据)。
  const stats = [
    { title: '用户总数', value: '—', icon: Users, hint: '待接入' },
    { title: '在线用户', value: '—', icon: Activity, hint: '待接入' },
    { title: '角色数', value: '—', icon: Shield, hint: '待接入' },
    { title: '缓存状态', value: '—', icon: Database, hint: '待接入' },
  ]

  let nickName = $derived(auth.user?.nickName || auth.user?.userName || '用户')
</script>

<div class="space-y-6 p-6">
  <div>
    <p class="text-muted-foreground mt-1 text-sm">这里是 {APP_NAME} 管理后台首页。</p>
  </div>

  <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
    {#each stats as s (s.title)}
      <Card.Root>
        <Card.Header class="flex flex-row items-center justify-between space-y-0 pb-2">
          <Card.Title class="text-sm font-medium">{s.title}</Card.Title>
          <s.icon class="text-muted-foreground h-4 w-4" />
        </Card.Header>
        <Card.Content>
          <div class="text-2xl font-bold">{s.value}</div>
          <p class="text-muted-foreground mt-1 text-xs">{s.hint}</p>
        </Card.Content>
      </Card.Root>
    {/each}
  </div>

  <Card.Root>
    <Card.Header>
      <Card.Title>开始使用</Card.Title>
    </Card.Header>
    <Card.Content class="text-muted-foreground space-y-2 text-sm">
      <p>左侧导航由后端菜单(<code>/system/menu/getRouters</code>)动态生成。</p>
      <p>点击菜单项进入对应管理页面。权限不足的菜单不会显示。</p>
    </Card.Content>
  </Card.Root>
</div>
