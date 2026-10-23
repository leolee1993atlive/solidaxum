import type { Component } from 'svelte'
import type { RouterVo } from '$lib/api/types.js'
import { resolveMenuIcon } from './icon-map.js'
import { hiddenRouteFor } from '$lib/stores/hidden-routes.js'
import type { HiddenRouteInfo } from '$lib/stores/hidden-routes.js'
import type { NavNode } from '$lib/components/layout/data/types.js'

/** 把顶层 path 归一化为以 / 开头(e.g. "system" → "/system", "/system" → "/system")。 */
export function normalizeBase(path: string): string {
	if (!path) return '/'
	return path.startsWith('/') ? path : `/${path}`
}

/** basePath + 子 path 拼接。子 path 若已是绝对路径(以 / 开头)则直接用。 */
export function joinPath(base: string, child: string): string {
	if (!child) return base
	if (child.startsWith('http://') || child.startsWith('https://')) return child
	if (child.startsWith('/')) return child
	const b = base.endsWith('/') ? base.slice(0, -1) : base
	return `${b}/${child}`
}

/** 把 getRouters 的 RouterVo 树转换为递归 NavNode[]。顶层目录亦为可折叠节点。 */
export function routersToNavTree(routers: RouterVo[]): NavNode[] {
	return routers
		.filter((r) => !r.hidden)
		.map((r) => routerToNode(r, ''))
}

function routerToNode(r: RouterVo, parentPath: string): NavNode {
	const title = r.meta?.title ?? r.name ?? r.path
	const icon = resolveMenuIcon(r.meta?.icon)

	// 外链:直接作为 url 节点
	if (r.meta?.link) return { title, url: r.meta.link, icon }

	// 本节点 full path:顶层归一化前导斜杠,子级 join 父路径
	const base = parentPath === '' ? normalizeBase(r.path) : joinPath(parentPath, r.path)
	const visibleChildren = (r.children ?? []).filter((c) => !c.hidden)

	if (visibleChildren.length === 0) {
		return { title, url: base, icon }
	}
	return {
		title,
		icon,
		items: visibleChildren.map((c) => routerToNode(c, base)),
	}
}

/** 保留 Component 类型引用,避免某些 lint 规则误报未使用 import。 */
export type { Component }

/**
 * 收集 getRouters 树中 hidden 叶子,产出 path -> { title, activeMenu } 表。
 * path 可能含 [param]/[[param]] 动态段,原样保留,由消费方(hiddenRouteFor)匹配。
 * 供 tab 标题取名与侧边栏 activeMenu 高亮用(对齐 Vue 的 route.meta 行为)。
 */
export function routersToHiddenMap(
	routers: RouterVo[]
): Record<string, HiddenRouteInfo> {
	const map: Record<string, HiddenRouteInfo> = {}
	const walk = (rs: RouterVo[], parentPath: string) => {
		for (const r of rs) {
			if (r.meta?.link) continue
			const base =
				parentPath === '' ? normalizeBase(r.path) : joinPath(parentPath, r.path)
			const children = (r.children ?? []).filter((c) => !c.meta?.link)
			if (children.length === 0) {
				if (r.hidden) {
					map[base] = {
						title: r.meta?.title ?? r.name ?? r.path,
						...(r.meta?.activeMenu ? { activeMenu: r.meta.activeMenu } : {}),
					}
				}
			} else {
				walk(children, base)
			}
		}
	}
	walk(routers, '')
	return map
}

/**
 * 隐藏/非菜单路由的标题兜底表(前缀匹配)。优先用 getRouters 隐藏菜单数据
 * (routersToHiddenMap 注入 hidden-routes store),查不到才走本表 —— 覆盖
 * 纯前端路由(如个人中心)或 DB 未播种对应隐藏菜单的旧库。
 * 新增纯前端隐藏页只需在此追加一行。[正则前缀, 标题]。
 */
const HIDDEN_TITLES: ReadonlyArray<readonly [RegExp, string]> = [
	[/^\/workflow\/design\/legacy$/, '流程设计器(旧)'],
	[/^\/workflow\/design(\/index)?$/, '流程设计器'],
	[/^\/tool\/gen-edit\/index/, '修改生成配置'],
	[/^\/system\/role-auth\/user\//, '分配用户'],
	[/^\/user\/profile/, '个人中心'],
]

/** 查隐藏路由标题表。按数组顺序,首个匹配前缀正则的胜出。 */
function hiddenTitle(pathname: string): string | undefined {
	for (const [re, title] of HIDDEN_TITLES) {
		if (re.test(pathname)) return title
	}
	return undefined
}

/**
 * 从 NavNode 树反查路径对应标题(供布局 <title> 用)。
 * 最长前缀匹配。菜单树未命中再查 getRouters 隐藏菜单表(hidden-routes store),
 * 最后兜底静态表 HIDDEN_TITLES。三者都未命中返回 undefined。
 */
export function pathToTitle(nodes: NavNode[], pathname: string): string | undefined {
	let best: { url: string; title: string } | undefined

	const consider = (url: string, title: string) => {
		if (pathname === url || pathname.startsWith(url + '/')) {
			if (!best || url.length > best.url.length) {
				best = { url, title }
			}
		}
	}

	const walk = (ns: NavNode[]) => {
		for (const n of ns) {
			if (n.url) consider(n.url, n.title)
			if (n.items) walk(n.items)
		}
	}

	walk(nodes)
	if (best) return best.title
	const hidden = hiddenRouteFor(pathname)
	if (hidden) return hidden.title
	return hiddenTitle(pathname)
}

/** 递归收集所有可点击叶子(含 url),供命令面板/搜索用。 */
export function flattenNavLinks(
	nodes: NavNode[]
): Array<{ title: string; url: string }> {
	const out: Array<{ title: string; url: string }> = []
	const walk = (ns: NavNode[]) => {
		for (const n of ns) {
			if (n.url && !/^https?:\/\//.test(n.url)) out.push({ title: n.title, url: n.url })
			if (n.items) walk(n.items)
		}
	}
	walk(nodes)
	return out
}
