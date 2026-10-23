@REPO_NAME@ 后端 - Windows 安装包(交互式向导)
=============================================

【内容】
  @REPO_NAME@-setup-<版本>-windows-x64.exe   交互式安装程序(Inno Setup)
  (zip 解压包已不再提供 —— 改用 setup.exe。遗留 zip 用户见下方「迁移」段)

【系统要求】
  - Windows 10 / Windows Server 2019 及更新,x86_64
  - 管理员权限(安装程序会自动提权)
  - 外部数据库(MySQL/Postgres/SQLite) 与 Redis(本包不包含数据库)

【安装(交互式向导)】
  1. 右键 @REPO_NAME@-setup-<版本>-windows-x64.exe → 以管理员身份运行
  2. 向导会依次收集:实例后缀 → 安装目录 → 服务端口 → 数据库 → Redis
  3. 点「下一步」到 Redis 页时,安装程序会:
     a. 调用后端 --write-config 自动渲染 config.toml(CSPRNG 生成
        jwt.secret 与 security.aes_key)
     b. 调用后端 --check-config 实测 DB+Redis 连通(不通会停在本页)
  4. 安装完成后自动注册 WinSW 服务、启动、探活 /api/health,
     成功则打开浏览器

  无需手动编辑 config.toml。

【升级(同服务名再跑 setup.exe)】
  向导会:
    - 从注册表检测到已安装版本,自动填入原目录
    - 从旧 config.toml 预填端口/DB/Redis 默认值
    - 先停止旧服务(轮询到 STOPPED,防 exe 文件占用)
    - 走外科手术式更新(保留 jwt.secret / security.aes_key 与手改的高级段)

【启停】
  Start-Service  @REPO_NAME@
  Stop-Service   @REPO_NAME@
  Get-Service    @REPO_NAME@

【日志】
  应用日志:<安装目录>\logs\
  系统日志:事件查看器 → 应用程序

【前端更新】
  解压前端发布包,用其内容覆盖 <安装目录>\web\
  静态文件下次请求即生效,无需重启服务。

【启用前端托管】
  config.toml 的 [server] 设 static_dir = "web"(综合包已默认带 web\ 目录)。

【卸载】
  控制面板 → 程序和功能 → @REPO_NAME@ → 卸载
  仅注销 Windows 服务并删除程序文件;
  config.toml / data / logs 保留在安装目录,需彻底清理请手工删除整个文件夹。

【从遗留 zip 包迁移到 setup.exe】
  旧版用 zip + install.ps1 装的机器:
  1. 在旧安装目录以管理员运行 uninstall.ps1(只注销服务,不删文件)
  2. 备份旧 config.toml(里面有已加密的 OSS 密钥)
  3. 运行新 setup.exe(安装目录选旧的,会自动从旧 config.toml 预填)

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

  其中 <端口> 为安装时填写的服务端口号(默认 3000)。
  配置完成后:
    nginx -t          # 检查配置语法
    nginx -s reload

【现场增量更新(-backend- zip)】
  @REPO_NAME@-backend-<版本>-windows-x64.zip 只含新的 @REPO_NAME@.exe,给
  「bug 修复、无配置/依赖变更」的场景用(全量更新仍走 setup.exe 重装):

    1. 停服务: services.msc 找到 @REPO_NAME@ 服务停止,或管理员执行
       net stop @REPO_NAME@
    2. 解压 zip,把 @REPO_NAME@.exe 覆盖到安装目录(服务注册的 exe 路径,
       默认 C:\Program Files\@REPO_NAME@\)
    3. 起服务: net start @REPO_NAME@(或 services.msc 里启动)
    4. 探活: curl http://127.0.0.1:<端口>/api/health

  注意:config.toml 不在 zip 里 —— 覆盖 exe 不会动配置;涉及配置结构变更
  的版本必须走 setup.exe 升级(它会用 toml_edit 做保留式配置迁移)。
