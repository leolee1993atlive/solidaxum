// 主力场景：读多型业务流，模拟真实管理后台的"查列表 + 看详情 + 拉部门/字典"。
// 用 constant-arrival-rate（恒定到达率）而非恒定并发，避免协调遗漏(coordinated
// omission)——后端变慢时请求会堆积而不是自动降速，长尾才真实。
//
// 关键设计：token 在 setup() 里登录一次后全程共享。这样压测的是"带鉴权的读接口"
// 本身，而不是 bcrypt 登录开销（登录吞吐另见 login.js）。两个后端都在每个请求里
// 用 token 查 Redis 校验会话，所以这对比依然公平。
//
//   k6 run -e BASE_URL=http://localhost:3000 -e RATE=500 -e DURATION=60s bench/k6/read_mix.js
import { login, makeSummary } from './lib/common.js';
import { readMixSetup, readMixIteration } from './lib/flows.js';

const RATE = Number(__ENV.RATE || 300); // 每秒请求数
const DURATION = __ENV.DURATION || '60s';
const PRE_VUS = Number(__ENV.PRE_VUS || 200);
const MAX_VUS = Number(__ENV.MAX_VUS || 1500);

export const options = {
  scenarios: {
    read_mix: {
      executor: 'constant-arrival-rate',
      rate: RATE,
      timeUnit: '1s',
      duration: DURATION,
      preAllocatedVUs: PRE_VUS,
      maxVUs: MAX_VUS,
    },
  },
  // 这是对比基准，不是验收测试，所以只留一个宽松的健康线（错误率），
  // 别用严格延迟阈值制造 exit code 噪音。延迟好坏看汇总里的 p95/p99 自行判断。
  thresholds: {
    http_req_failed: ['rate<0.05'],
  },
  // 让汇总和导出的 JSON 都带上 p99（k6 默认只到 p95）
  summaryTrendStats: ['avg', 'min', 'med', 'p(90)', 'p(95)', 'p(99)', 'max'],
  setupTimeout: '30s',
};

export function setup() {
  const token = login();
  return { token, userId: readMixSetup(token) };
}

export default function (data) {
  readMixIteration(data);
}

// 落盘 JSON（供 compare.ps1）+ 打印精简文本，逻辑共享自 common.js。
export function handleSummary(data) {
  return makeSummary(data);
}
