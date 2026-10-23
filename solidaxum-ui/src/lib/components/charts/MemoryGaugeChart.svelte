<script lang="ts">
  import * as echarts from 'echarts/core'
  import { GaugeChart } from 'echarts/charts'
  import { TooltipComponent } from 'echarts/components'
  import { CanvasRenderer } from 'echarts/renderers'
  import { mode } from 'mode-watcher'

  echarts.use([GaugeChart, TooltipComponent, CanvasRenderer])

  let {
    used,
    max,
    label,
    height = '420px',
  }: {
    /** 已用字节 */
    used: number
    /** 刻度上限字节(maxmemory,为 0 时调用方回落 used_memory_peak) */
    max: number
    /** 表盘中央显示的文本,即 redis 的 used_memory_human */
    label: string
    height?: string
  } = $props()

  let el = $state<HTMLDivElement | null>(null)
  let chart: echarts.ECharts | null = null

  function render() {
    if (!el) return
    chart?.dispose()
    chart = echarts.init(el, mode.current === 'dark' ? 'dark' : undefined, {
      renderer: 'canvas',
    })
    // 上限兜底:max 为 0(未配 maxmemory 且 peak 也取不到)时用 used 本身,
    // 让指针落在满格而不是除零。
    const scaleMax = max > 0 ? max : Math.max(used, 1)
    chart.setOption({
      backgroundColor: 'transparent',
      tooltip: { formatter: `{b} <br/>{a} : ${label}` },
      series: [
        {
          name: '峰值',
          type: 'gauge',
          min: 0,
          max: scaleMax,
          // 刻度是字节,轴标签直接显示会是一串长数字 —— 关掉,
          // 有意义的数字由 detail 的 used_memory_human 承担(与 Vue 一致)。
          axisLabel: { show: false },
          detail: { formatter: () => label, fontSize: 18 },
          data: [{ value: used, name: '内存消耗' }],
        },
      ],
    })
  }

  $effect(() => {
    void used
    void max
    void label
    void mode.current
    render()
  })

  $effect(() => {
    if (typeof window === 'undefined') return
    const onResize = () => chart?.resize()
    window.addEventListener('resize', onResize)
    return () => {
      window.removeEventListener('resize', onResize)
      chart?.dispose()
      chart = null
    }
  })
</script>

<div bind:this={el} style:height class="w-full"></div>
