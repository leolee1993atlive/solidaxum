// 写场景：考验事务、唯一索引，以及 PG 的 WAL 与 MySQL 的 redo。
//
// 配比:
//   50% PUT /system/user/changeStatus —— 单列更新,最轻
//   30% PUT /system/user              —— 多表更新(sys_user + sys_user_role + sys_user_post)
//   20% POST /system/notice           —— 单表插入,走事务与 WAL/redo
//
// 插入档原先是 POST /system/user,已换掉:建用户要算 bcrypt,40 迭代/秒下
// 那 20% 就吃掉约 0.84 核,写场景实际测的是 bcrypt 而不是事务/WAL/redo/
// 唯一索引(第一轮报告 §7.6)。bcrypt 本身的对比由 login 场景承担 ——
// 那里两边算的是同一个 cost 10 的 hash,是最干净的一次隔离。
//
// 换 notice 而不是 dict/data:RuoYi 的字典接口带 @CacheEvict,会额外打
// Redis,引入新的不对等。notice 两侧都是干净的单表插入。
//
// 每轮开始前数据库会被完整回灌(bench/lib/dbreset.ps1),所以插入的行不需要
// 脚本自己清理,也不会跨轮冲突。
//
// 角色/岗位用数据集自带的 990001/990002 —— 种子里的角色 ID 全是雪花号且
// sys_post 一行都没有,写死 2/4 会指向不存在的行(见 bench/seed/gen-seed-sql.ps1)。
//
//   k6 run -e BASE_URL=http://127.0.0.1:3000/api -e RATE=50 -e DURATION=60s bench/k6/write_mix.js
import http from 'k6/http';
import { fail } from 'k6';
import {
  BASE_URL,
  login,
  authHeaders,
  bizOk,
  makeSummary,
} from './lib/common.js';
import { benchUser, writeMixIteration } from './lib/flows.js';

const RATE = Number(__ENV.RATE || 50);
const DURATION = __ENV.DURATION || '60s';
const PRE_VUS = Number(__ENV.PRE_VUS || 100);
const MAX_VUS = Number(__ENV.MAX_VUS || 800);

export const options = {
  scenarios: {
    write_mix: {
      executor: 'constant-arrival-rate',
      rate: RATE,
      timeUnit: '1s',
      duration: DURATION,
      preAllocatedVUs: PRE_VUS,
      maxVUs: MAX_VUS,
    },
  },
  thresholds: { http_req_failed: ['rate<0.05'] },
  summaryTrendStats: ['avg', 'min', 'med', 'p(90)', 'p(95)', 'p(99)', 'max'],
  setupTimeout: '30s',
};

// 压测用户是**构造出来的**,不必查列表就能算出来。
// 对应 bench/seed/gen-seed-sql.ps1:
//   user_id   = 1000000 + i             (i = 1..200000)
//   user_name = 'bench_u' + i 补零到 6 位
//   dept_id   = 900 + ((i * 37) % 100) + 1
//
// 早先靠 GET /system/user/list 取目标用户,撞上两个坑:
//   1) pageSize 有上限 —— 请求 500 实测只返回 200
//   2) **预热本身就是 write_mix**:60 秒 @ 25/s 会创建约 300 个新用户,
//      而 axum 按 user_id DESC 排序,新用户的雪花 ID(7.5e18)远大于压测段,
//      把压测用户整个挤出首页 → 正式采样的 setup() 报「未取到压测用户」,
//      k6 以 exit=107 让整轮作废。批次 6 里 axum-pg / axum-pgb 六轮就这么废了。
// 直接算就完全不受排序、分页上限、以及自己刚建的用户影响。
const USER_POOL = Number(__ENV.USER_POOL || 200000);

// 新建用户的名字里必须带一个**每次运行都不同**的随机段。
//
// 原因:一轮压测里冒烟 / 预热 / 正式采样是三个独立的 k6 进程,VU 与 ITER
// 都从头计数。若用户名只由 (VU, ITER) 决定,预热创建的 bench_new_1_7 会和
// 正式采样要创建的 bench_new_1_7 撞名 —— 后者被后端拒绝。
// 实测:批次 6 里 axum-pg 的「8.33% 错误」全部来自这个重名,
// 去掉预热重跑即 0%。
//
// 这个假象还差点被写成结论:RuoYi 拒绝重名时返回 HTTP 200 + body code:500,
// 而 axum 返回非 2xx —— k6 的 http_req_failed 只看状态码,于是同样的业务
// 失败在 axum 记作错误、在 RuoYi 记作成功。两边真实的业务失败率其实一模一样
// (check 通过率 91.67% vs 91.68%)。
const RUN_ID = Math.random().toString(36).slice(2, 6);

export function setup() {
  const token = login();
  // 只验证数据集在位,不依赖列表取目标用户
  const first = benchUser(1);
  const probe = http.get(`${BASE_URL}/system/user/${first.userId}`, {
    headers: authHeaders(token),
  });
  if (!bizOk(probe)) {
    fail(
      `压测用户 ${first.userId} 取不到(code=${probe.json('code')})。` +
        '数据集没灌进去?先跑 bench/seed/provision.ps1。',
    );
  }
  return { token };
}

export default function (data) {
  writeMixIteration(data, RUN_ID, USER_POOL);
}

export function handleSummary(data) {
  return makeSummary(data);
}
