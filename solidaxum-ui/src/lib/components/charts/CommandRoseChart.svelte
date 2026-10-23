<script lang="ts">
  import * as echarts from 'echarts/core'
  import { PieChart } from 'echarts/charts'
  import { TooltipComponent, LegendComponent } from 'echarts/components'
  import { CanvasRenderer } from 'echarts/renderers'
  import { mode } from 'mode-watcher'

  // 按需注册,避免把整个 echarts 打进包(全量约 1MB,这里只要 pie)。
  echarts.use([PieChart, TooltipComponent, LegendComponent, CanvasRenderer])

  let {
    data = [],
    height = '420px',
  }: {
    data: { name: string; value: number }[]
    height?: string
  } = $props()

  let el = $state<HTMLDivElement | null>(null)
  let chart: echarts.ECharts | null = null

  function render() {
    if (!el) return
    // 主题切换后要重建实例:ECharts 的 theme 只能在 init 时传。
    chart?.dispose()
    chart = echarts.init(el, mode.current === 'dark' ? 'dark' : undefined, {
      renderer: 'canvas',
    })
    chart.setOption({
      backgroundColor: 'transparent',
      tooltip: {
        trigger: 'item',
        // 逐字对齐 Vue index.vue:177(a=series名 b=命令 c=次数 d=占比)
        formatter: '{a} <br/>{b} : {c} ({d}%)',
      },
      series: [
        {
          name: '命令',
          type: 'pie',
          roseType: 'radius',
          radius: [15, 95],
          center: ['50%', '50%'],
          data,
          animationEasing: 'cubicInOut',
          animationDuration: 1000,
        },
      ],
    })
  }

  $effect(() => {
    // 读这两个以建立响应式依赖:数据变或主题变都要重画。
    void data
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
