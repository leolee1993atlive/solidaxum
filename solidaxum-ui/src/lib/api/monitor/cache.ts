import { apiClient } from '$lib/api/client.js'

/** Redis 缓存监控 CacheInfo —— 对齐后端 R<CacheInfo> (单对象,非分页)。 */
export interface CacheInfo {
  info: Record<string, string>
  dbSize: number
  /** 后端 handler.rs:44-46 每项固定为 {name, value},勿写成 {command, calls}。 */
  commandStats: { name: string; value: string | number }[]
}

/**
 * 命令统计 → ECharts pie series data。
 * 键名保持后端的 {name, value},因为 ECharts 的 pie series 就吃这两个键
 * —— 与 Vue 直接把 commandStats 丢给 setOption 的做法等价(index.vue:181)。
 * ⚠️ 不截断:CA2 此前的 TOP_N=10 会把多余命令丢掉。
 */
export function commandStatsSeries(
  stats: CacheInfo['commandStats']
): { name: string; value: number }[] {
  return (stats ?? [])
    .map((c) => ({ name: c.name, value: Number(c.value) || 0 }))
    .sort((a, b) => b.value - a.value)
}

export const cacheApi = {
  get: () => apiClient.get<CacheInfo>('/monitor/cache'),
}

/**
 * redis INFO 里的字节数字段(used_memory / maxmemory / used_memory_peak)
 * 一律是十进制字节的纯数字串。非法值归零,避免仪表盘出现 NaN / 负指针。
 */
export function parseMemoryBytes(raw: string | undefined): number {
  const n = Number(raw)
  return Number.isFinite(n) && n > 0 ? n : 0
}

export type OverviewItem = { label: string; value: string }

/**
 * 缓存概览 12 项。标签、顺序、值转换逐条对齐 Vue index.vue:15-108。
 * 后端 parse_cache_info(cache/handler.rs:23) 把 redis INFO 全量键值
 * 平铺进 info,所以这些 key 都直接可取。
 */
export function cacheOverview(data: CacheInfo | null): OverviewItem[] {
  const info = data?.info ?? {}
  // 缺字段一律 '-' —— 不让面板出现 undefined,也不让整格塌掉。
  const g = (k: string): string => {
    const v = info[k]
    return v === undefined || v === '' ? '-' : String(v)
  }
  const cpu = Number(info.used_cpu_user_children)
  const netIn = info.instantaneous_input_kbps
  const netOut = info.instantaneous_output_kbps

  return [
    { label: 'Redis版本', value: g('redis_version') },
    {
      label: '运行模式',
      value:
        info.redis_mode === undefined
          ? '-'
          : info.redis_mode === 'standalone'
            ? '单机'
            : '集群',
    },
    { label: '端口', value: g('tcp_port') },
    { label: '客户端数', value: g('connected_clients') },
    { label: '运行时间(天)', value: g('uptime_in_days') },
    { label: '使用内存', value: g('used_memory_human') },
    {
      label: '使用CPU',
      value:
        Number.isFinite(cpu) && info.used_cpu_user_children !== undefined
          ? cpu.toFixed(2)
          : '-',
    },
    { label: '内存配置', value: g('maxmemory_human') },
    {
      label: 'AOF是否开启',
      value:
        info.aof_enabled === undefined
          ? '-'
          : info.aof_enabled === '0'
            ? '否'
            : '是',
    },
    { label: 'RDB是否成功', value: g('rdb_last_bgsave_status') },
    { label: 'Key数量', value: data ? String(data.dbSize) : '-' },
    {
      label: '网络入口/出口',
      value:
        netIn === undefined || netOut === undefined
          ? '-'
          : `${netIn}kps/${netOut}kps`,
    },
  ]
}
