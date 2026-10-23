import { describe, it, expect } from 'vitest'
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'

// 扫描 src 下全部 .svelte 源码,守住令牌用法与表单弹层规则,防止回潮。
// happy-dom 下 new URL(import.meta.url) 解析有怪癖,沿用仓库惯例 process.cwd()。
const SRC = join(process.cwd(), 'src')

function svelteFiles(dir: string, acc: string[] = []): string[] {
	for (const entry of readdirSync(dir)) {
		const full = join(dir, entry)
		if (statSync(full).isDirectory()) svelteFiles(full, acc)
		else if (entry.endsWith('.svelte')) acc.push(full)
	}
	return acc
}

const files = svelteFiles(SRC).map((f) => ({ path: f, text: readFileSync(f, 'utf8') }))

describe('主色文字用法', () => {
	it('不再出现裸 text-primary(填充配套的 text-primary-foreground 不受影响)', () => {
		const offenders = files
			.filter((f) => /\btext-primary\b(?!-)/.test(f.text))
			.map((f) => f.path.replace(SRC, ''))
		expect(offenders).toEqual([])
	})
})

describe('语义色令牌化', () => {
	it('业务组件不再硬编码 amber / emerald 色阶', () => {
		const offenders = files
			.filter((f) => !f.path.includes('components/ui/'))
			.filter((f) => /(?:text|bg|border)-(?:amber|emerald)-\d{3}/.test(f.text))
			.map((f) => f.path.replace(SRC, ''))
		expect(offenders).toEqual([])
	})
})

describe('warning/success 文字用法', () => {
	it('业务组件不再出现裸 text-warning / text-success(填充用途 bg-/border- 不受影响)', () => {
		const offenders = files
			.filter((f) => !f.path.includes('components/ui/'))
			.filter((f) => /\btext-(warning|success)\b(?!-)/.test(f.text))
			.map((f) => f.path.replace(SRC, ''))
		expect(offenders).toEqual([])
	})
})

describe('裸 hex 颜色', () => {
	it('业务组件 class/style 属性内不再出现裸 #rrggbb(颜色一律走令牌;ui 原语与皮肤目录豁免)', () => {
		const offenders = files
			.filter((f) => !f.path.includes('components/ui/'))
			.filter((f) => !f.path.includes('themes/'))
			.filter((f) => /(?:class|style)="[^"]*#[0-9a-fA-F]{6}/.test(f.text))
			.map((f) => f.path.replace(SRC, ''))
		expect(offenders).toEqual([])
	})
})

/**
 * 业务页 CRUD/编辑表单一律走共享 FormDrawer(风格平价计划 D1 裁定,自下游
 *参考实现回流)。白名单 = 存量手写抽屉,新增业务表单禁止再手写 Sheet.Root。
 */
// user-view-drawer 为只读详情抽屉,收编与否单独裁定,暂不强制。
const SHEET_ROOT_WHITELIST = new Set<string>(['routes/(app)/system/user/user-view-drawer.svelte'])

describe('业务表单弹层统一 FormDrawer', () => {
	it('业务页不再新增手写 Sheet.Root(白名单内为待收编存量)', () => {
		const offenders = files
			.filter((f) => f.path.includes(join('routes', '(app)')))
			.filter((f) => f.text.includes('Sheet.Root'))
			.map((f) => f.path.replace(SRC, '').replaceAll('\\', '/').replace(/^\//, ''))
			.filter((p) => !SHEET_ROOT_WHITELIST.has(p))
		expect(offenders).toEqual([])
	})
})
