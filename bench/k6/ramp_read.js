// 爬坡定区间:一轮内从低到高线性加压,找拐点大致落在哪一档。
//
// 结果**不进报告正文**(设计 §3.1)——每个瞬时点样本少、资源采样对不上
// 具体速率,只用来决定批次 8/9 的阶梯档位。报告里引用的数只能来自阶梯轮次。
//
// 不设 thresholds:爬坡本来就是要压到不达标为止,阈值只会制造 exit=99 噪音,
// 而 run.ps1 把 99 当成「合法的过载结果」记录——爬坡轮次不需要那个标记。
//
//   k6 run -e BASE_URL=... -e RATE=4000 -e DURATION=300s bench/k6/ramp_read.js
import { login, makeSummary } from './lib/common.js';
import { readMixSetup, readMixIteration } from './lib/flows.js';

// RATE 在爬坡脚本里的含义是**终点速率**,不是恒定速率。沿用同一个变量名
// 是为了让 run.ps1 不必为爬坡另开一条参数通路,轮次标签 r<终点速率> 也仍可读。
const TARGET = Number(__ENV.RATE || 2000);
const STEPS = Number(__ENV.RAMP_STEPS || 10);
const STEP_DUR = __ENV.RAMP_STEP || '30s';
const PRE_VUS = Number(__ENV.PRE_VUS || 200);
const MAX_VUS = Number(__ENV.MAX_VUS || 4000);

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
  return { token, userId: readMixSetup(token) };
}

export default function (data) {
  readMixIteration(data);
}

export function handleSummary(data) {
  return makeSummary(data);
}
