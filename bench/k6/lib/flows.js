// 读/写场景一次迭代的请求组合。恒定速率脚本与爬坡脚本共用 —— 爬坡指出的
// 拐点要能对上阶梯的档位,两个脚本就必须打完全相同的请求,这件事由共用
// 一份代码保证,而不是靠人盯两处。
import http from 'k6/http';
import { check } from 'k6';
import exec from 'k6/execution';
import {
  BASE_URL,
  authHeaders,
  bizOk,
  rowsOf,
  postJson,
  putJson,
} from './common.js';

// 取一个真实存在的 userId 供详情接口使用。
export function readMixSetup(token) {
  const list = http.get(`${BASE_URL}/system/user/list?pageNum=1&pageSize=10`, {
    headers: authHeaders(token),
  });
  const first = rowsOf(list)[0];
  return (first && first.userId) || 1;
}

export function readMixIteration(data) {
  const h = { headers: authHeaders(data.token) };

  // 60% 列表查询(带翻页,制造一点查询多样性)
  const page = 1 + (__ITER % 5);
  const list = http.get(
    `${BASE_URL}/system/user/list?pageNum=${page}&pageSize=10`,
    { ...h, tags: { name: 'GET /system/user/list' } },
  );
  check(list, { 'list ok': (r) => bizOk(r) });

  // 25% 详情
  if (__ITER % 4 === 0) {
    const d = http.get(`${BASE_URL}/system/user/${data.userId}`, {
      ...h,
      tags: { name: 'GET /system/user/{id}' },
    });
    check(d, { 'detail ok': (r) => bizOk(r) });
  }

  // 15% 部门树 / 字典等低频读
  if (__ITER % 6 === 0) {
    const dept = http.get(`${BASE_URL}/system/dept/list`, {
      ...h,
      tags: { name: 'GET /system/dept/list' },
    });
    check(dept, { 'dept ok': (r) => bizOk(r) });
  }
}

// 数据集自带的压测角色与岗位。种子里的角色 ID 全是雪花号且 sys_post
// 一行都没有,写死 2/4 会指向不存在的行(见 bench/seed/gen-seed-sql.ps1)。
export const BENCH_ROLE_ID = 990001;
export const BENCH_POST_ID = 990002;
const DEPT_BASE = 900;
const DEPT_COUNT = 100;

// 压测用户是**构造出来的**,不必查列表就能算出来。对应
// bench/seed/gen-seed-sql.ps1 的三条生成规则。
export function benchUser(i) {
  return {
    userId: 1000000 + i,
    userName: 'bench_u' + String(i).padStart(6, '0'),
    deptId: DEPT_BASE + ((i * 37) % DEPT_COUNT) + 1,
  };
}

export function writeMixIteration(data, runId, userPool) {
  // 取值必须与 k6 分配了多少 VU **无关**。
  //
  // 原先是 (__VU * 31 + __ITER * 7) % userPool。constant-arrival-rate 按需
  // 分配 VU:后端越快,维持同一速率需要的 VU 越少,31·VU 这一项的跨度就越窄,
  // 索引挤在一小段里,重复请求体随之增多 —— 而 changeStatus 的请求体只有
  // {userId, status},撞上就被防重复提交拒掉。
  //
  // 实测 1800 迭代/秒:axum 只用到 100 个 VU,changeStatus 被拒 19404 次;
  // RuoYi 慢,被迫扩到 800 个 VU,只被拒 30 次。**这是在惩罚更快的一方**,
  // 测出来的写吞吐差距完全是这个耦合造出来的。
  //
  // iterationInTest 是本场景内跨 VU 的全局迭代序号,与 VU 数量无关;
  // 乘一个质数把相邻迭代打散,避免多个 VU 在同一时刻盯住相邻行
  // (那会让多表更新的加锁顺序交叉,产生 MySQL 死锁 —— 原注释要防的正是这个)。
  const n = exec.scenario.iterationInTest;
  const pick = benchUser(1 + ((n * 7919) % userPool));
  const slot = n % 10;

  if (slot < 5) {
    // 50% —— 单列更新。状态在 0/1 之间来回翻,不会单调漂移。
    const r = putJson(
      `${BASE_URL}/system/user/changeStatus`,
      data.token,
      { userId: pick.userId, status: slot % 2 === 0 ? '1' : '0' },
      'PUT /system/user/changeStatus',
    );
    check(r, { 'changeStatus ok': (x) => bizOk(x) });
  } else if (slot < 8) {
    // 30% —— 多表更新:sys_user 之外还要重写 sys_user_role / sys_user_post
    const r = putJson(
      `${BASE_URL}/system/user`,
      data.token,
      {
        userId: pick.userId,
        // userName 必填 —— 缺了 axum 直接返回
        // 「JSON 解析失败: missing field userName」400
        userName: pick.userName,
        deptId: pick.deptId,
        nickName: `压测改名${__VU}_${__ITER}`,
        email: `upd${__VU}_${__ITER}@example.com`,
        phonenumber: '13900000000',
        sex: '0',
        status: '0',
        roleIds: [BENCH_ROLE_ID],
        postIds: [BENCH_POST_ID],
      },
      'PUT /system/user',
    );
    check(r, { 'update ok': (x) => bizOk(x) });
  } else {
    // 20% —— 单表插入。RuoYi 侧 noticeTitle 有 @Size(max = 50)。
    const uniq = `${runId}_${__VU}_${__ITER}`;
    const r = postJson(
      `${BASE_URL}/system/notice`,
      data.token,
      {
        noticeTitle: `bench_${uniq}`,
        noticeType: '1',
        noticeContent: `压测公告 ${uniq}`,
        status: '0',
      },
      'POST /system/notice',
    );
    check(r, { 'notice create ok': (x) => bizOk(x) });
  }
}
