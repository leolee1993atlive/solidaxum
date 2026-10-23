import { LayoutGrid } from '@lucide/svelte'
import type { Component } from 'svelte'

import { APP_NAME } from '$lib/brand.js'

/**
 * 品牌信息(应用名/图标)——静态常量,与后端菜单无关。
 * 动态菜单由 auth context 的 navTree(来自 /system/menu/getRouters)驱动,
 * 见 src/lib/menu/transform.ts。APP_NAME 已迁至 $lib/brand.ts。
 */
export { APP_NAME }
export const APP_ICON: Component = LayoutGrid

// SidebarData 类型仍供需要品牌信息的消费者使用(navTree 改为动态注入)。
export type { SidebarData } from './types'
