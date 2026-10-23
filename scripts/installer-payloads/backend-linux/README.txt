@REPO_NAME@ 后端 - Linux 安装包
====================================

【内容】
  @REPO_NAME@                               后端二进制(单文件,动态链接 unixODBC)
  web/                                      前端 SPA 静态文件(由后端进程托管;
                                            仅全量包含 —— 包名带 -backend- 段的
                                            纯后端包没有此目录,前端用 -ui- zip
                                            另行更新)
  templates/systemd/@REPO_NAME@.service     systemd 单元模板
  install.sh / uninstall.sh                 安装 / 卸载脚本(交互式)
  VERSION                                   构建版本与 git sha

【系统要求】
  - Linux x86_64,glibc 2.28+(openEuler 20+ / RHEL 8+ / Ubuntu 20.04+ / Debian 11+)
  - systemd
  - root 权限
  - 外部 MySQL/Postgres/SQLite/达梦(DM8)与 Redis(本包不包含数据库)
  - unixODBC 运行库(libodbc.so.2):yum install unixODBC / apt install unixodbc
    ⚠ 无论用哪种数据库都需要 —— 二进制固定动态链接 unixODBC(达梦 ODBC 客户端所致)。
      达梦驱动 libdodbc.so 不在二进制内,需按下方【达梦(DM8)额外要求】另行安装。

  ⚠ CentOS 7 / RHEL 7(glibc 2.17)不在本包覆盖范围。
    低 glibc 环境请走 Docker 部署(见 docker/ 包),docker 镜像自带
    新版 glibc/libstdc++,宿主机内核 3.10+ 即可运行。

  【达梦(DM8)额外要求】
    - 达梦 ODBC 驱动已注册到 /etc/odbcinst.ini
    - 数据源已注册到 /etc/odbc.ini 并配好 SERVER / TCP_PORT
      (安装向导里填的「ODBC 数据源名」就是这里的条目名,不是库名)
    - 建表脚本已执行:solidaxum_init.sql(从源码仓库 sql/dm/ 目录获取,详见该目录 Readme.md)
    - 建库参数 CASE_SENSITIVE=N(建库后不可改)

【安装(交互式向导)】
  # 把整个 @REPO_NAME@/ 目录 mv 到你希望服务运行的位置:
  sudo mv @REPO_NAME@ /opt/        # 或 /srv/、/var/lib/、…… 自选
  cd /opt/@REPO_NAME@
  sudo ./install.sh

  脚本会交互式收集端口/数据库/Redis 配置,自动:
    1. 调用后端 --write-config 渲染 config.toml(CSPRNG 自动生成
       jwt.secret 与 security.aes_key)
    2. 调用后端 --check-config 实测 DB+Redis 连通(不通则循环重问)
    3. 注册 systemd 服务并以指定服务用户身份运行
    4. 启动服务并探活 /api/health

  无需手动编辑 config.toml。

【升级(同服务名再跑 install.sh)】
  sudo ./install.sh
  脚本会:
    - 检测到已注册实例,从旧 config.toml 预填默认值
    - 停止旧服务,备份 config.toml 到 config.toml.bak-<时间戳>
    - 走外科手术式更新(保留 jwt.secret / security.aes_key 与手改的高级段)

【启停】
  sudo systemctl start  @REPO_NAME@
  sudo systemctl stop   @REPO_NAME@
  sudo systemctl status @REPO_NAME@

【日志】
  sudo journalctl -u @REPO_NAME@ -f
  应用层日志在 <prefix>/logs/

【迁移目录】
  安装后不要直接 mv 目录,systemd 持有绝对路径会失效。正确做法:
    sudo <prefix>/uninstall.sh        # 注销 systemd(文件不删)
    sudo mv <prefix> <new-prefix>
    cd <new-prefix> && sudo ./install.sh

【前端单独更新】
  收到新版 @REPO_NAME@-web-<VERSION>-linux-x64.zip 时:
    unzip @REPO_NAME@-web-<VERSION>-linux-x64.zip
    sudo rsync -a --delete web/ <prefix>/web/
    sudo chown -R app:app <prefix>/web
  不需要 systemctl restart。

【卸载】
  sudo <prefix>/uninstall.sh
  只注销 systemd,目录下的二进制 / web / data / logs / config.toml 全部保留,
  请按需手工 rm -rf <prefix>。

【常见问题】
  - 安装时 "数据库连接失败":检查 DB 地址/端口/防火墙,脚本会循环重问直到连通
  - 启动失败 "permission denied" on data/:chown -R app:app <prefix>/data
  - 装在 /home/<user>/ 下后服务起不来:本模板已默认关掉 ProtectHome;
    如果你手动加回了 ProtectHome=true,在 /home 下会被 systemd 拦截

【Nginx 反向代理配置(可选)】
  若需要将特定接口代理到 @REPO_NAME@ 后端,在 Nginx 配置中添加:

  location ^~ /api/ {
      proxy_pass              http://localhost:<端口>/api/;
      proxy_set_header        Host $http_host;
      proxy_set_header        X-Real-IP $remote_addr;
      proxy_set_header        X-Forwarded-For $proxy_add_x_forwarded_for;
      proxy_set_header        Upgrade $http_upgrade;
      proxy_set_header        Connection "upgrade";

      proxy_connect_timeout 100s;
      proxy_send_timeout 1800s;
      proxy_read_timeout 1800s;
      send_timeout 1800s;
  }

  其中 <端口> 为 config.toml 中 [server] 配置的端口号(默认 3000)。
  配置完成后:
    sudo nginx -t          # 检查配置语法
    sudo systemctl reload nginx
