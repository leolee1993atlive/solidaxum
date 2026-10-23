import type { Component } from 'svelte'

// Navigation types
export type NavItem = {
  title: string
  badge?: string
  icon?: Component
}

/**
 * 递归导航节点。叶子有 url、无 items;目录有 items、无 url。
 * 顶层目录也是 NavNode(可折叠),支持任意层级。
 */
export type NavNode = NavItem & {
  url?: string
  items?: NavNode[]
}

export type SidebarData = {
  appName: string
  appIcon: Component
}
