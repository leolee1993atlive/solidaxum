# 性能压测：axum vs RuoYi-Vue-Plus

语言无关的 HTTP 层压测。axum 是 RuoYi-Vue-Plus 的 Rust 移植，接口完全一致，
所以**同一份 k6 脚本只换 `BASE_URL` 就能打两个后端**，对比绝对公平。

工具用 [k6](https://k6.io/)：脚本化、支持登录态、自带 p50/p95/p99 与吞吐汇总。

## 目录

| 文件 | 作用 |
|------|------|
| `k6/lib/common.js` | 共享：登录拿 token、鉴权头 |
| `k6/smoke.js`      | 冒烟，1 VU 跑通核心接口（正式压测前必跑） |
| `k6/read_mix.js`   | **主力场景**：读多型业务流（列表/详情/部门），恒定到达率 |
| `k6/login.js`      | 登录吞吐（bcrypt + Redis 会话，最能拉开 Rust/JVM 差距） |
| `run.ps1`          | 对单后端"预热 + 正式采样"，结果落盘 |
| `compare.ps1`      | 并排对比两后端最近一次结果 |
| `results/`         | 每次运行的 JSON 汇总 |

## 0. 装 k6

```powershell
winget install k6
# 或 choco install k6 ；其他平台见 https://k6.io/docs/get-started/installation/
```

## 1. 关掉验证码（两边都要，否则登录失败）

压测脚本不解验证码。两个后端都得先关：

- **axum**：`solidaxum-rs/config.toml` 里 `[captcha] enable = false`，重启。
- **RuoYi-Vue-Plus**：默认验证码开关在 `sys_config` 表的 `sys.account.captchaEnabled`，
  改成 `false`（或在系统参数里关），无需重启。

> 也可以另存一份 `config.bench.toml` 专用于压测，避免动开发配置。

## 2. 公平对比的前提（务必检查）

1. **压测机 ≠ 被测机**：k6 和后端别挤同一台机器抢 CPU。本机自测时至少确认 CPU 没打满。
2. **预热**：`run.ps1` 默认先跑 20s 预热再采样——RuoYi 是 Spring Boot，JIT 没热起来很吃亏。
3. **同一个 MySQL、同样的数据量**：两个后端连同一库、同样行数，否则比的是 DB 不是后端。
4. **同时盯服务端资源**：压测时另开窗口看两边 CPU/内存
   （`docker stats` 或后端自带的 `/metrics`）。同样 RPS 下 Rust 通常 CPU/内存低得多，
   这往往比裸 RPS 更有说服力。

## 3. 跑

```powershell
# 先冒烟，确认两边都通
k6 run -e BASE_URL=http://localhost:3000 bench/k6/smoke.js   # axum
k6 run -e BASE_URL=http://localhost:8080 bench/k6/smoke.js   # ruoyi

# 读多型场景：每秒 500 请求，跑 60s
./bench/run.ps1 -Target axum  -Scenario read_mix -Rate 500 -Duration 60s
./bench/run.ps1 -Target ruoyi -Scenario read_mix -Rate 500 -Duration 60s

# 对比
./bench/compare.ps1
```

登录吞吐(bcrypt 很重，速率调低)：

```powershell
./bench/run.ps1 -Target axum  -Scenario login -Rate 50 -Duration 60s
./bench/run.ps1 -Target ruoyi -Scenario login -Rate 50 -Duration 60s
./bench/compare.ps1
```

远程压测换地址：`-BaseUrl http://192.168.1.10:3000`。

### 自动拉起后端（-AutoStart）

不想手动开后端时，加 `-AutoStart`：脚本会自己启动后端、等就绪、压测、结束后停掉。
**已在运行的实例会被复用、不会被停**（避免误杀你手动起的）。

```powershell
# axum：自动 cargo build --release（性能对比必须 release，debug 慢数倍）后启动
./bench/run.ps1 -Target axum  -Scenario read_mix -Rate 100 -Duration 30s -AutoStart

# RuoYi：首次用 mvnw 打 ruoyi-admin.jar（数分钟，之后缓存），再 java -jar 启动
./bench/run.ps1 -Target ruoyi -Scenario read_mix -Rate 100 -Duration 30s -AutoStart

# 想压测后保持后端运行：-KeepUp；指定 JDK：-Jdk C:\path\to\jdk21
```

自启实现见 [lib/backends.ps1](lib/backends.ps1)。两个踩过的坑已内置处理：
- **RuoYi 自动关 p6spy**（`--spring.datasource.dynamic.p6spy=false`）。dev profile 默认开它，
  会把每条 SQL 同步打到（被重定向的）stderr，并发下所有线程堵在写日志上 →
  延迟假性飙到十几秒。配置注释自己都写"有性能损耗,不建议生产"。
- **预热用低速**（`-WarmupRate 25`，默认）而非测试速率，避免全速打冷 JVM 造成假过载。

## ⚠️ 当前是"远程库瓶颈"，不是纯框架对比

dev 数据源如果指向远程 MySQL,每条 SQL 都有网络往返(~30ms),一个列表请求要串行
多条 SQL,Hikari 池 20 连接 → 50/s 就排到 ~1s 延迟。axum 若连同一个远程库则**两边公平**,
但结论会被网络 DB 延迟主导,框架本身的差异被压缩,且结果随网络波动。

**要做干净的框架级对比**：让两个后端都连**同一个本地 MySQL**（同样的数据），
再压测。否则你测到的主要是到那台远程库的网络延迟。

## 4. 怎么读结果

`compare.ps1` 输出关注三列：

- **RPS** —— 实际达成吞吐。注意:`read_mix` 用的是**恒定到达率**，若后端扛不住
  目标速率，达成 RPS 会低于设定值，且 p99 飙升——这正是要找的拐点。
- **P95 / P99 延迟** —— 长尾。比平均值重要得多。
- **ErrorPct** —— 超过 0~1% 说明已过载或配置有问题，此时延迟数据不可信。

### 找拐点的正确姿势

不要只跑一个固定 RATE。阶梯式抬:`-Rate 200 → 500 → 1000 → 2000`，
观察哪个点开始 p99 暴涨 / 错误率上升 / 达成 RPS 不再跟随。**那个点就是该后端的容量上限**，
两个后端的拐点对比，比任何单点数字都有意义。
