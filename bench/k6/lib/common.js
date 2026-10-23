// 两个后端共用的工具函数。
// axum 是 RuoYi-Vue-Plus 的 Rust 移植，接口完全一致，所以这份脚本
// 只靠 BASE_URL 环境变量切换被测目标，保证对比公平。
import http from 'k6/http';
import { check, fail } from 'k6';

// ---- 可调参数（全部走环境变量，默认对准本机 axum）----
// 注意：用 BENCH_ 前缀，别用裸 USERNAME/PASSWORD —— k6 的 __ENV 会暴露所有 OS
// 环境变量，而 Windows 内置 USERNAME=当前登录名，会把默认值顶掉。
export const BASE_URL = __ENV.BASE_URL || 'http://localhost:3000';
export const USERNAME = __ENV.BENCH_USER || 'admin';
export const PASSWORD = __ENV.BENCH_PASS || 'admin123';
// RuoYi-Vue-Plus PC 端默认 clientId
export const CLIENT_ID = __ENV.CLIENT_ID || 'e5cd7e4891bf95d1d19206ce24a7b32e';

const JSON_HEADERS = { 'Content-Type': 'application/json' };

// 登录一次，返回 access_token。失败直接终止（避免后续全 401 污染数据）。
// 前提：被测后端已关闭验证码，否则这里会因缺 code/uuid 而失败。
export function login() {
  const res = http.post(
    `${BASE_URL}/auth/login`,
    JSON.stringify({
      clientId: CLIENT_ID,
      grantType: 'password',
      username: USERNAME,
      password: PASSWORD,
      // 随机 uuid 让每次登录的**请求体不同**。
      //
      // 必须这么做:axum 的防重复提交是全局中间件,键是 md5(token + body);
      // 登录时 token 为空、账号密码固定,所以一轮里的冒烟 / 预热 / 正式采样
      // 三次 k6 进程各登录一次,后两次会落在 5 秒窗口内被判重复提交
      //   {"code":500,"msg":"不允许重复提交,请稍候再试"}
      // —— setup() 直接失败,整轮作废。
      //
      // 为什么不像 login.js 那样换账号:admin 是超管,数据权限能看到全部
      // 20 万行;压测账号的角色 data_scope='2' 只能看本部门,换账号会
      // **改变查询负载**,读场景就不可比了。
      //
      // 为什么 uuid 安全:它是验证码校验用的字段,两个后端的 LoginBody 都有,
      // 关掉验证码时都会直接忽略(axum: validate_captcha 立即返回 Ok)。
      // 既不改变服务端做的活,又能让摘要不同。
      uuid: `${Date.now()}-${Math.random().toString(36).slice(2)}`,
    }),
    { headers: JSON_HEADERS, tags: { name: 'POST /auth/login' } },
  );

  // 先判请求是否真的拿到了响应。请求层失败(连不上/超时)时 res.body 为 null，
  // 直接 res.json() 会抛 "body is null" 的晦涩错误，这里给出清晰提示。
  if (res.status === 0 || res.body === null) {
    fail(
      `登录请求未拿到响应(status=${res.status}, error=${res.error || res.error_code})：` +
        `后端 ${BASE_URL} 可能没起来或没就绪。先确认后端已就绪(axum: ${BASE_URL}/health;RuoYi: ${BASE_URL}/auth/code)。`,
    );
  }

  const ok = check(res, {
    'login 200': (r) => r.status === 200,
    'login 拿到 token': (r) => !!(r.body && r.json('data.access_token')),
  });
  if (!ok) {
    fail(
      `登录失败 (status=${res.status})：请确认后端已关闭验证码、账号密码正确。响应=${res.body}`,
    );
  }
  return res.json('data.access_token');
}

export function authHeaders(token) {
  // clientid 是 RuoYi-Vue-Plus 的硬要求：Sa-Token 鉴权会校验 header 里的 clientid
  // 与 token 内存的 clientId 一致，否则 200 包 401「登录状态异常」。axum 端不校验
  // 这个 header，多带无害，所以两边共用。
  return {
    Authorization: `Bearer ${token}`,
    clientid: CLIENT_ID,
    ...JSON_HEADERS,
  };
}

// 业务成功判定：两个后端都用 {code,...} 包裹，RuoYi 鉴权失败时 HTTP 仍是 200，
// 只有 body.code 才反映真相。光看 r.status 会假阳性。
export function bizOk(r) {
  return r.status === 200 && r.json('code') === 200;
}

// 取分页 rows，兼容两种结构：RuoYi 套在 data.rows，axum 也包在 R 的 data 里；
// 个别实现可能放顶层 rows。挨个兜底，谁有用谁。
export function rowsOf(r) {
  return r.json('data.rows') || r.json('rows') || [];
}

// 各场景共用的汇总处理：落盘 JSON（供 compare.ps1）+ 打印精简文本。
// OUTDIR 由 run.ps1 传绝对路径，避免 k6 相对路径找不到目录。
export function makeSummary(data) {
  const tag = __ENV.TAG || 'run';
  const dir = (__ENV.OUTDIR || 'bench/results').replace(/\\/g, '/');
  return {
    [`${dir}/${tag}.summary.json`]: JSON.stringify(data, null, 2),
    stdout: textSummary(data),
  };
}

function textSummary(data) {
  const m = data.metrics;
  const p = (metric, stat) =>
    m[metric] && m[metric].values[stat] !== undefined
      ? m[metric].values[stat].toFixed(2)
      : 'n/a';
  const reqs = m.http_reqs ? m.http_reqs.values.count : 0;
  const rps = m.http_reqs ? m.http_reqs.values.rate.toFixed(1) : 'n/a';
  const failRate = m.http_req_failed
    ? (m.http_req_failed.values.rate * 100).toFixed(2)
    : 'n/a';
  return [
    '',
    `===== ${__ENV.TAG || 'run'} @ ${BASE_URL} =====`,
    `总请求:   ${reqs}   吞吐: ${rps} req/s   错误率: ${failRate}%`,
    `延迟(ms): avg ${p('http_req_duration', 'avg')}  ` +
      `p50 ${p('http_req_duration', 'med')}  ` +
      `p95 ${p('http_req_duration', 'p(95)')}  ` +
      `p99 ${p('http_req_duration', 'p(99)')}  ` +
      `max ${p('http_req_duration', 'max')}`,
    '',
  ].join('\n');
}

// 带鉴权的 JSON POST/PUT。写场景(write_mix.js)专用 —— 读场景只用 GET。
export function postJson(url, token, body, name) {
  return http.post(url, JSON.stringify(body), {
    headers: authHeaders(token),
    tags: { name },
  });
}

export function putJson(url, token, body, name) {
  return http.put(url, JSON.stringify(body), {
    headers: authHeaders(token),
    tags: { name },
  });
}
