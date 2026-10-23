import { describe, expect, it } from 'vitest'
import { commandStatsSeries, parseMemoryBytes, cacheOverview } from './cache.js'

describe('commandStatsSeries', () => {
  it('按后端字段 {name,value} 取值并按次数降序', () => {
    const stats = [
      { name: 'get', value: '5' },
      { name: 'set', value: 12 },
      { name: 'del', value: '3' },
    ]
    expect(commandStatsSeries(stats)).toEqual([
      { name: 'set', value: 12 },
      { name: 'get', value: 5 },
      { name: 'del', value: 3 },
    ])
  })

  // CA2:此前 TOP_N=10 把多余命令截断丢弃,即便 CA1 修好仍在丢数据。
  // Vue 把 commandStats 整个交给 ECharts,一条不丢。
  it('不截断:输入多少条就返回多少条', () => {
    const stats = Array.from({ length: 25 }, (_, i) => ({
      name: `cmd${i}`,
      value: i,
    }))
    expect(commandStatsSeries(stats)).toHaveLength(25)
  })

  it('非数字 value 归零而非 NaN', () => {
    expect(commandStatsSeries([{ name: 'get', value: 'x' }])).toEqual([
      { name: 'get', value: 0 },
    ])
  })

  it('空输入返回空数组', () => {
    expect(commandStatsSeries([])).toEqual([])
  })
})

describe('parseMemoryBytes', () => {
  it('纯数字字节串直接解析', () => {
    expect(parseMemoryBytes('1048576')).toBe(1048576)
  })

  it('undefined / 空串 / 非数字归零', () => {
    expect(parseMemoryBytes(undefined)).toBe(0)
    expect(parseMemoryBytes('')).toBe(0)
    expect(parseMemoryBytes('abc')).toBe(0)
  })

  it('负数归零(不让仪表盘出现负指针)', () => {
    expect(parseMemoryBytes('-1')).toBe(0)
  })
})

describe('cacheOverview', () => {
  const base = {
    dbSize: 42,
    commandStats: [],
    info: {
      redis_version: '7.2.4',
      redis_mode: 'standalone',
      tcp_port: '6379',
      connected_clients: '3',
      uptime_in_days: '12',
      used_memory_human: '1.05M',
      used_cpu_user_children: '0.123456',
      maxmemory_human: '0B',
      aof_enabled: '0',
      rdb_last_bgsave_status: 'ok',
      instantaneous_input_kbps: '0.12',
      instantaneous_output_kbps: '3.45',
    },
  }

  it('返回 12 项,顺序对齐 Vue index.vue:15-108', () => {
    const items = cacheOverview(base)
    expect(items).toHaveLength(12)
    expect(items.map((i) => i.label)).toEqual([
      'Redis版本', '运行模式', '端口', '客户端数',
      '运行时间(天)', '使用内存', '使用CPU', '内存配置',
      'AOF是否开启', 'RDB是否成功', 'Key数量', '网络入口/出口',
    ])
  })

  it('standalone 译成「单机」,其余译成「集群」', () => {
    expect(cacheOverview(base)[1].value).toBe('单机')
    expect(
      cacheOverview({ ...base, info: { ...base.info, redis_mode: 'cluster' } })[1].value,
    ).toBe('集群')
  })

  it('CPU 保留两位小数', () => {
    expect(cacheOverview(base)[6].value).toBe('0.12')
  })

  it("aof_enabled 为 '0' 是「否」,其余是「是」", () => {
    expect(cacheOverview(base)[8].value).toBe('否')
    expect(
      cacheOverview({ ...base, info: { ...base.info, aof_enabled: '1' } })[8].value,
    ).toBe('是')
  })

  it('网络入出口合并成一格', () => {
    expect(cacheOverview(base)[11].value).toBe('0.12kps/3.45kps')
  })

  it('data 为 null 时 12 项仍在,值一律 -(不塌成空面板)', () => {
    const items = cacheOverview(null)
    expect(items).toHaveLength(12)
    expect(items.every((i) => i.value === '-')).toBe(true)
  })

  it('缺字段用 - 兜底而不是 undefined', () => {
    const items = cacheOverview({ dbSize: 0, commandStats: [], info: {} })
    expect(items[0].value).toBe('-')
  })
})
