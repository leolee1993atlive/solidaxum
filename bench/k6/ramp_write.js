// 写场景的爬坡。与 ramp_read.js 同一形状,迭代体来自 lib/flows.js ——
// 爬坡指出的拐点要能对上批次 9 的阶梯,两个脚本必须打完全相同的请求。
//
// 结果不进报告正文(设计 §3.1)。
//
//   k6 run -e BASE_URL=... -e RATE=800 -e DURATION=300s bench/k6/ramp_write.js
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

// RATE 在爬坡脚本里的含义是**终点速率**,不是恒定速率。
const TARGET = Number(__ENV.RATE || 800);
const STEPS = Number(__ENV.RAMP_STEPS || 10);
const STEP_DUR = __ENV.RAMP_STEP || '30s';
const PRE_VUS = Number(__ENV.PRE_VUS || 100);
const MAX_VUS = Number(__ENV.MAX_VUS || 2000);
const USER_POOL = Number(__ENV.USER_POOL || 200000);

// 与 write_mix.js 同理:冒烟/预热/正式采样是独立的 k6 进程,VU 与 ITER
// 都从头计数,不带随机段会跨进程产生同名数据。
const RUN_ID = Math.random().toString(36).slice(2, 6);

const stages = [];
for (let i = 1; i <= STEPS; i++) {
  stages.push({ target: Math.round((TARGET * i) / STEPS), duration: STEP_DUR });
}

export const options = {
  scenarios: {
    ramp: {
      executor: 'ramping-arrival-rate',
      startRate: Math.round(TARGET / STEPS),
      timeUnit: '1s',
      preAllocatedVUs: PRE_VUS,
      maxVUs: MAX_VUS,
      stages,
    },
  },
  thresholds: {},
  summaryTrendStats: ['avg', 'min', 'med', 'p(90)', 'p(95)', 'p(99)', 'max'],
  setupTimeout: '30s',
};

export function setup() {
  const token = login();
  // 只验证数据集在位,不依赖列表取目标用户:列表有 pageSize 上限,
  // 且自己刚建的行会按 ID 排序把压测段挤出首页。
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
