// 冒烟脚本：1 个虚拟用户跑一遍核心接口，确认登录态和路径都通。
// 正式压测前先跑它，避免把"配置错误"当成"性能差"。
//   k6 run -e BASE_URL=http://localhost:3000 bench/k6/smoke.js
import { check, sleep } from 'k6';
import http from 'k6/http';
import {
  BASE_URL,
  login,
  authHeaders,
  bizOk,
  rowsOf,
  postJson,
} from './lib/common.js';

export const options = { vus: 1, iterations: 1 };

export default function () {
  const token = login();
  const h = { headers: authHeaders(token) };

  const list = http.get(
    `${BASE_URL}/system/user/list?pageNum=1&pageSize=10`,
    { ...h, tags: { name: 'GET /system/user/list' } },
  );
  check(list, {
    'list ok': (r) => bizOk(r),
    'list 有 rows': (r) => rowsOf(r).length > 0,
  });

  const firstId = rowsOf(list)[0] && rowsOf(list)[0].userId;
  if (firstId) {
    const detail = http.get(`${BASE_URL}/system/user/${firstId}`, {
      ...h,
      tags: { name: 'GET /system/user/{id}' },
    });
    check(detail, { 'detail ok': (r) => bizOk(r) });
  }

  const dept = http.get(`${BASE_URL}/system/dept/list`, {
    ...h,
    tags: { name: 'GET /system/dept/list' },
  });
  check(dept, { 'dept ok': (r) => bizOk(r) });

  // 写场景的插入档打的是这个接口。权限缺失(system:notice:add)、字段名不符、
  // 标题超长在这里露头,比在正式轮次里表现为「业务成功率低」要早得多。
  // 标题必须短:RuoYi 侧 noticeTitle 有 @Size(max = 50)。
  const notice = postJson(
    `${BASE_URL}/system/notice`,
    token,
    {
      noticeTitle: `smoke_${Date.now()}`,
      noticeType: '1',
      noticeContent: '冒烟公告',
      status: '0',
    },
    'POST /system/notice',
  );
  check(notice, { 'notice create ok': (r) => bizOk(r) });

  sleep(0.2);
}
