import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { joinPath, normalizeBase } from './transform.js'

// 真相源 A:Svelte 路由目录树(src/routes/(app)/ 下所有 +page.svelte)
const ROUTES_DIR = join(process.cwd(), 'src', 'routes', '(app)')
// 真相源 B:后端的全量初始化 SQL(monorepo 内 solidaxum-rs/sql/,原 sys_menu_seed.sql 已并入此文件)。
// 前后端同仓耦合是有意的(设计 §4.4)——任一侧改菜单/路由都应在 CI 被发现;
// 文件不存在即测试失败(强制种子在仓库内)。
const SEED_REL = join('solidaxum-rs', 'sql', 'solidaxum_init.sql')

// git worktree 下 cwd 会比仓库根深若干层,写死的 '..' 会指偏。逐级向上找
// 第一个含该 SQL 的目录;找不到则回落到规范位置(兄弟目录),让下面的
// 存在性断言报出一条人能看懂的路径。
function resolveSeedSql(): string {
	for (let dir = process.cwd(); ; ) {
		const candidate = join(dir, SEED_REL)
		if (existsSync(candidate)) return candidate
		const parent = dirname(dir)
		if (parent === dir) return join(process.cwd(), '..', SEED_REL)
		dir = parent
	}
}
const SEED_SQL = resolveSeedSql()

// 设计 §4.2 白名单:这些路由存在但不走 sys_menu 驱动,允许无菜单
const NON_MENU_ROUTES = new Set([
	'/index', // 首页/dashboard,固定入口
	'/user/profile', // 个人中心,固定入口
	'/workflow/design/legacy', // 旧 iframe 设计器,C-4 过渡期并存页,菜单仍指到新页
])

// 扫描 +page.svelte 得到实际路由路径(以 / 开头,统一 posix 分隔符)。
// win32 下 readdir 拼出的路径是反斜杠,比较前必须归一化。
function walkPages(dir: string, out: string[] = []): string[] {
	for (const name of readdirSync(dir)) {
		const p = join(dir, name)
		if (statSync(p).isDirectory()) walkPages(p, out)
		else if (name === '+page.svelte') {
			out.push(
				p
					.slice(ROUTES_DIR.length)
					.replace(/[/\\]\+page\.svelte$/, '')
					.replaceAll('\\', '/')
			)
		}
	}
	return out
}

// 归一化:动态段 [param]/[[optional]] 截断到静态前缀,两边都归一后再比对
// (种子里写的是模板 '/system/role-auth/user/[roleId]',实际 URL 带具体值)。
function normalizeDynamic(p: string): string {
	return p.replace(/\[+\w+\]+.*$/, '').replace(/\/$/, '')
}

// 按 SQL 字符串字面量切分单条 VALUES 元组(引号内的逗号不当分隔符)。
// 当前种子已核实没有任何带逗号的引号字段,此处仍按字面量切分以防未来漂移静默错位。
// 带引号字段取去引号后的内容('' 与 \' 两种转义都认);裸 NULL 归一为空串
// (新种子里 component/query_param/active_menu 等列是裸 NULL 而非 '')。
function splitValues(tuple: string): string[] {
	const out: string[] = []
	let cur = ''
	let quoted = false
	let inQuote = false
	const flush = () => {
		const raw = cur.trim()
		out.push(!quoted && raw.toUpperCase() === 'NULL' ? '' : raw)
		cur = ''
		quoted = false
	}
	for (let i = 0; i < tuple.length; i++) {
		const ch = tuple[i]
		if (inQuote) {
			if (ch === '\\') cur += tuple[++i] ?? '' // mysqldump 默认的反斜杠转义
			else if (ch === "'") {
				if (tuple[i + 1] === "'") {
					cur += "'" // SQL 的 '' 转义
					i++
				} else inQuote = false
			} else cur += ch
		} else if (ch === "'") {
			inQuote = true
			quoted = true
			cur = '' // 丢掉引号前的空白
		} else if (ch === ',') flush()
		else cur += ch
	}
	flush()
	return out
}

// 从 values 关键字之后扫出该 INSERT 的所有元组(逐个 depth-0 括号对),到语句
// 结束的分号为止。引号内的括号/分号不参与计数,故支持跨行的多行 VALUES。
function scanTuples(src: string, from: number): string[] {
	const tuples: string[] = []
	let depth = 0
	let start = 0
	let inQuote = false
	for (let i = from; i < src.length; i++) {
		const ch = src[i]
		if (inQuote) {
			if (ch === '\\') i++
			else if (ch === "'") {
				if (src[i + 1] === "'") i++
				else inQuote = false
			}
		} else if (ch === "'") inQuote = true
		else if (ch === '(') {
			if (depth === 0) start = i + 1
			depth++
		} else if (ch === ')') {
			depth--
			if (depth === 0) tuples.push(src.slice(start, i))
		} else if (ch === ';' && depth === 0) break
	}
	return tuples
}

interface SeedRow {
	pid: string
	path: string
	name: string
}

// 解析种子 SQL 的 M/C 型菜单,按 parent_id 建树,复用 transform.ts 的
// normalizeBase/joinPath 逐层拼出完整 path 链(与运行时 getRouters 同一语义)。
// 返回 Map<归一化后的完整 path, 菜单名>。
// 取值一律按 INSERT 自带的列名列表定位,不按位置硬编码 —— 后端换 dump 工具或调
// 列序都不会静默错位。结构对不上(找不到 INSERT / 缺列)直接抛,避免解析失败退
// 化成空 Map 假绿。
const NEEDED_COLS = ['menu_id', 'menu_name', 'parent_id', 'path', 'menu_type', 'is_frame'] as const

function parseSeedMenuPaths(sqlPath: string): Map<string, string> {
	const src = readFileSync(sqlPath, 'utf8')
	const rows = new Map<string, SeedRow>()

	// 反引号表名 + 显式列名列表 + 跨行多行 VALUES。整表可能被 dump 成多条 INSERT,
	// 故全局扫描而非只取第一条(漏掉后半段会让幽灵菜单断言假绿)。
	const stmt = /insert\s+into\s+`?sys_menu`?\s*\(([^)]*)\)\s*values/gi
	let found = 0
	for (let m = stmt.exec(src); m; m = stmt.exec(src)) {
		found++
		const cols = new Map(
			m[1].split(',').map((c, i) => [c.trim().replace(/`/g, '').toLowerCase(), i] as const)
		)
		for (const name of NEEDED_COLS) {
			if (!cols.has(name)) throw new Error(`种子 SQL 的 sys_menu INSERT 缺列 ${name}: ${sqlPath}`)
		}
		for (const tuple of scanTuples(src, stmt.lastIndex)) {
			const fields = splitValues(tuple)
			const f = (name: (typeof NEEDED_COLS)[number]) => fields[cols.get(name)!] ?? ''
			if (f('menu_type') === 'F') continue // F 型按钮不对应路由
			if (f('is_frame') === '0') continue // is_frame='0' 是外链(本仓 0=外链,与 Vue 的 'Y' 相反),不对应内部路由
			const path = f('path')
			if (path === '' || path === '#') continue // 无路径占位,不对应路由
			rows.set(f('menu_id'), { pid: f('parent_id'), path, name: f('menu_name') })
		}
	}
	if (found === 0) throw new Error(`种子 SQL 里找不到 sys_menu 的 INSERT: ${sqlPath}`)

	// 与 transform.ts routerToNode 同语义:顶层 normalizeBase,子级 joinPath 父链。
	const memo = new Map<string, string>()
	function fullPath(id: string, visiting: Set<string> = new Set()): string {
		const cached = memo.get(id)
		if (cached !== undefined) return cached
		const row = rows.get(id)
		if (!row || visiting.has(id)) return '' // 断链/环:防御,种子评审已确认不存在
		visiting.add(id)
		const full = row.pid === '0' ? normalizeBase(row.path) : joinPath(fullPath(row.pid, visiting), row.path)
		memo.set(id, full)
		return full
	}

	const result = new Map<string, string>()
	for (const id of rows.keys()) {
		const row = rows.get(id)!
		result.set(normalizeDynamic(fullPath(id)), row.name)
	}
	return result
}

describe('菜单 path ↔ 路由 漂移护栏', () => {
	it('种子 SQL 可读且真的解析出了菜单(强制全量种子在 monorepo 的 solidaxum-rs/sql/ 内)', () => {
		expect(() => readFileSync(SEED_SQL, 'utf8'), `种子 SQL 不可读: ${SEED_SQL}`).not.toThrow()
		// 下面两条断言都以「解析出的菜单集」为前提:若 SQL 形态漂移导致一条都没解析出来,
		// 幽灵菜单断言会空集通过(假绿)。此处兜住,让解析失败自己现形。
		const menuPaths = parseSeedMenuPaths(SEED_SQL)
		expect(menuPaths.size, `种子 SQL 未解析出任何菜单(SQL 形态漂移?): ${SEED_SQL}`).toBeGreaterThan(0)
	})

	it('无孤儿路由(Svelte 有页面但 sys_menu 无菜单入口)', () => {
		const routes = walkPages(ROUTES_DIR)
		const menuPaths = parseSeedMenuPaths(SEED_SQL)
		const orphans = routes
			.filter((r) => !NON_MENU_ROUTES.has(r))
			.map(normalizeDynamic)
			.filter((r) => !menuPaths.has(r))
			.sort()
		expect(orphans, `孤儿路由(无菜单入口): ${orphans.join(', ')}`).toEqual([])
	})

	it('无幽灵菜单(sys_menu 有菜单但 Svelte 无路由,点了 404)', () => {
		const routes = new Set(walkPages(ROUTES_DIR).map(normalizeDynamic))
		const menuPaths = parseSeedMenuPaths(SEED_SQL)
		const ghosts: string[] = []
		for (const [path, name] of menuPaths) {
			// 允许菜单 path 是某路由的静态前缀(如 /workflow 匹配 /workflow/category)
			const matched = routes.has(path) || [...routes].some((r) => r.startsWith(`${path}/`))
			if (!matched) ghosts.push(`${path} (${name})`)
		}
		ghosts.sort()
		expect(ghosts, `幽灵菜单(点了 404): ${ghosts.join(', ')}`).toEqual([])
	})
})
