import type { Component } from 'svelte'
import {
	Activity,
	Bell,
	BookOpen,
	Boxes,
	Briefcase,
	Brush,
	CalendarClock,
	ChevronRight,
	CloudUpload,
	Code2,
	Database,
	Edit3,
	FileInput,
	FileSpreadsheet,
	FileText,
	Folder,
	FolderTree,
	GitBranch,
	Globe,
	HelpCircle,
	History,
	LayoutDashboard,
	LayoutGrid,
	Library,
	ListTodo,
	Lock,
	LogIn,
	type LucideIcon,
	Network,
	Shield,
	Star,
	User,
	Users,
	Wrench,
} from '@lucide/svelte'

/**
 * RuoYi 后端菜单 icon 是其自带 SVG 图标名(字符串),Svelte 侧用 lucide。
 * 本表把常见 RuoYi icon 名映射到 lucide 组件;未命中用 LayoutGrid 默认。
 */
const ICON_MAP: Record<string, Component> = {
	// 系统管理类
	system: LayoutGrid,
	user: User,
	peoples: Users,
	people: User,
	'tree-table': Network,
	tree: Network,
	post: Briefcase,
	dict: Library,
	edit: Edit3,
	message: Bell,
	'notice': Bell,
	log: FileText,
	international: Globe,
	client: Users,
	lock: Lock,
	// 监控类
	monitor: Activity,
	online: User,
	logininfor: History,
	logininfo: History,
	'job': CalendarClock,
	redis: Database,
	cache: Database,
	operlog: History,
	snailjob: CalendarClock,
	// 工具类
	tool: Wrench,
	code: Code2,
	gen: Code2,
	build: Brush,
	// 工作流类
	workflow: GitBranch,
	category: FolderTree,
	'process-definition': FileText,
	'process-instance': Boxes,
	waiting: ListTodo,
	finish: History,
	'my-task': ListTodo,
	'my-copy': FileText,
	// 表单/上传/仪表盘类 —— 实测真实库 sys_menu.icon 在用但此前未映射,
	// 未映射时侧边栏会显示成默认方块。
	input: FileInput,
	form: FileSpreadsheet,
	upload: CloudUpload,
	dashboard: LayoutDashboard,
	// 通用
	star: Star,
	guide: HelpCircle,
	checkbox: ChevronRight,
}

/**
 * 把后端菜单 icon 字符串解析为 lucide 组件。
 * 未映射的 icon 名(含 '#' 空图标占位)回落到 LayoutGrid。
 */
export function resolveMenuIcon(iconName?: string): Component {
	if (!iconName || iconName === '#' || iconName.trim() === '') return LayoutGrid
	return ICON_MAP[iconName] ?? LayoutGrid
}

/**
 * 可选图标名全集(供菜单页的图标选择器用)。
 *
 * 🔴 选项**必须**就是 ICON_MAP 的键 —— 给一个表里没有的名字,存进去之后
 * resolveMenuIcon 会回落 LayoutGrid,侧边栏显示成默认方块,等于选了个寂寞。
 * 新增图标请先往 ICON_MAP 里加,这里自动就有了。
 */
export function menuIconNames(): string[] {
	return Object.keys(ICON_MAP).sort()
}

/** 按关键词过滤图标名(子串、大小写不敏感)。空关键词返回全部。 */
export function filterIconNames(keyword: string): string[] {
	const k = keyword.trim().toLowerCase()
	if (k === '') return menuIconNames()
	return menuIconNames().filter((n) => n.includes(k))
}

// 让 LucideIcon 类型在需要时可用(部分组件签名约束)
export type { LucideIcon }
