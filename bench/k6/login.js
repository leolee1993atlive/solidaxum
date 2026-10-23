// 登录吞吐场景:每次迭代都跑完整登录(含 bcrypt 校验密码 + 写 Redis 会话)。
// 这条最能拉开 Rust 与 JVM 的差距,也最考验连接池/线程模型,所以单独成一个场景,
// 速率要比纯读低(bcrypt 很重,别拿读接口的 RATE 直接套)。
//
// **每次迭代用不同的压测账号**,不是反复登录 admin。三个理由:
//   1) axum 的防重复提交是全局中间件,对所有 POST/PUT 生效(含 /auth/login);
//      RuoYi 的 @RepeatSubmit 是注解级,登录接口上没有。若每次请求体都相同,
//      axum 侧 5 秒窗口内第二次登录起会全部被拒,登录压测根本做不了。
//      换账号后请求体天然不同,摘要不碰撞 —— 不必为压测关掉这个特性,
//      两边都保持真实行为。
//   2) 更接近真实系统:生产环境是很多人各自登录,不是一个账号被打 4500 次。
//   3) 绕开单账号在 Redis 上的会话键热点,避免测成单键争用。
//
// 20 万个压测账号(bench_u000001 ~ bench_u200000)口令统一为 admin123,
// 且共用同一个 bcrypt hash(cost 10),所以两边算的活完全一样。
//
//   k6 run -e BASE_URL=http://127.0.0.1:3000/api -e RATE=50 -e DURATION=60s bench/k6/login.js
import http from 'k6/http';
import { check } from 'k6';
import { CLIENT_ID, BASE_URL, PASSWORD, makeSummary } from './lib/common.js';

const RATE = Number(__ENV.RATE || 50);
const DURATION = __ENV.DURATION || '60s';
// 账号池大小。取够大即可:只要一轮内不重复用同一账号,就不会撞防重复提交窗口。
const USER_POOL = Number(__ENV.USER_POOL || 200000);

export const options = {
  scenarios: {
    login_only: {
      executor: 'constant-arrival-rate',
      rate: RATE,
      timeUnit: '1s',
      duration: DURATION,
      preAllocatedVUs: Number(__ENV.PRE_VUS || 100),
      maxVUs: Number(__ENV.MAX_VUS || 800),
    },
  },
  thresholds: {
    http_req_failed: ['rate<0.05'],
  },
  summaryTrendStats: ['avg', 'min', 'med', 'p(90)', 'p(95)', 'p(99)', 'max'],
};

const JSON_HEADERS = { 'Content-Type': 'application/json' };

export default function () {
  // (VU, ITER) 唯一确定一个账号,同一轮内不重复。
  // 乘 977(与 200000 互质)把访问打散到整个 user_id 区间,
  // 避免顺序扫描造成不真实的索引/缓存局部性。
  const n = ((__VU * 100003 + __ITER) * 977) % USER_POOL;
  const username = 'bench_u' + String(n + 1).padStart(6, '0');

  const res = http.post(
    `${BASE_URL}/auth/login`,
    JSON.stringify({
      clientId: CLIENT_ID,
      grantType: 'password',
      username,
      password: PASSWORD,
    }),
    { headers: JSON_HEADERS, tags: { name: 'POST /auth/login' } },
  );
  check(res, {
    'login 200': (r) => r.status === 200,
    // 业务码也要看:RuoYi 鉴权/业务失败时 HTTP 仍是 200
    'login code 200': (r) => r.json('code') === 200,
    'login 有 token': (r) => !!r.json('data.access_token'),
  });
}

export function handleSummary(data) {
  return makeSummary(data);
}
