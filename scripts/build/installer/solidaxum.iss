; @REPO_NAME@ Windows 安装包(交互式向导)
; 由 scripts/build/build-windows.ps1 调用 ISCC 编译,勿手工双击编译。
;   /DMyVersion=1.0.0  /DMySourceDir=<staging\backend 的绝对路径>
;   /DMyRepoName=solidaxum  /DMyAppName="solidaxum"
;
; 差异:无 duckdb.dll(纯 Rust 无原生库依赖);@REPO_NAME@ 占位符动态化;
; 默认值对齐 axum config.toml.example。

#ifndef MyVersion
  #error 必须通过 /DMyVersion=<版本> 传入版本号
#endif
#ifndef MySourceDir
  #error 必须通过 /DMySourceDir=<staging\backend 路径> 传入源目录
#endif
#ifndef MyRepoName
  #define MyRepoName "solidaxum"
#endif
#ifndef MyAppName
  #define MyAppName "solidaxum"
#endif

#define BaseAppId "{7A3B1C2D-4E5F-6A7B-8C9D-0E1F2A3B4C5D}"

[Setup]
AppId={code:GetAppId}
AppName={#MyAppName}
AppVersion={#MyVersion}
AppPublisher=solidaxum
DefaultDirName={code:GetDefaultDir}
DisableProgramGroupPage=yes
UsePreviousLanguage=no
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#MySourceDir}\..\..\..\..\dist
OutputBaseFilename={#MyRepoName}-setup-{#MyVersion}-windows-x64
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
UninstallDisplayName={#MyAppName} {#MyVersion}

[Languages]
Name: "zh"; MessagesFile: "compiler:Default.isl"

[Files]
; ---- dontcopy 的二进制必须排在最前 ----
; SolidCompression 下 ExtractTemporaryFile 要连带解压排在它前面的所有文件,
; 放最前面才不会让向导点「下一步」时卡十几秒。
; (solidaxum 无 duckdb.dll,只需提取 exe)
Source: "{#MySourceDir}\{#MyRepoName}.exe"; Flags: dontcopy noencryption

; ---- 正式铺设的文件 ----
Source: "{#MySourceDir}\{#MyRepoName}.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#MySourceDir}\WinSW-x64.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#MySourceDir}\VERSION"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#MySourceDir}\README.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#MySourceDir}\templates\*"; DestDir: "{app}\templates"; Flags: ignoreversion recursesubdirs
Source: "{#MySourceDir}\web\*"; DestDir: "{app}\web"; Flags: ignoreversion recursesubdirs

[UninstallRun]
Filename: "{app}\service\{code:GetServiceName}.exe"; Parameters: "stop"; Flags: runhidden; RunOnceId: "StopSvc"
Filename: "{app}\service\{code:GetServiceName}.exe"; Parameters: "uninstall"; Flags: runhidden; RunOnceId: "UninstallSvc"

[Code]
var
  SuffixPage: TInputQueryWizardPage;
  PortPage: TInputQueryWizardPage;
  DbPage: TInputQueryWizardPage;
  DbTypeCombo: TNewComboBox;
  DbHelpLabel: TNewStaticText;
  RedisPage: TInputQueryWizardPage;
  IsUpgrade: Boolean;
  ExistingDir: String;

function GetSuffix(): String;
begin
  if SuffixPage = nil then
    Result := ''
  else
    Result := Trim(SuffixPage.Values[0]);
end;

function GetAppId(Param: String): String;
begin
  Result := '{#BaseAppId}';
  if GetSuffix() <> '' then
    Result := Result + '-' + GetSuffix();
end;

// 安装时把服务名落盘,卸载时读回。
// 卸载进程不会调用 InitializeWizard,SuffixPage 恒为 nil,后缀无从推导。
function ServiceNameFile(): String;
begin
  Result := ExpandConstant('{app}\service\service-name.txt');
end;

function GetServiceName(Param: String): String;
var
  Lines: TArrayOfString;
begin
  // Setup 进程:向导页在,由后缀推导。
  if SuffixPage <> nil then
  begin
    Result := '{#MyRepoName}';
    if GetSuffix() <> '' then
      Result := Result + '-' + GetSuffix();
    exit;
  end;

  // 卸载进程:只能读安装时落下的标记文件。读不到就退回默认名,
  // 至少让无后缀的正式环境能正常卸载。
  Result := '{#MyRepoName}';
  if LoadStringsFromFile(ServiceNameFile(), Lines)
     and (GetArrayLength(Lines) > 0) and (Trim(Lines[0]) <> '') then
    Result := Trim(Lines[0]);
end;

// ---------- 服务状态 ----------
// sc.exe 的输出在中文 Windows 上也是英文(实测 zh-CN 下为 "STATE : 4 RUNNING"),
// 因此可以安全地按英文关键字解析。
function ServiceExists(Svc: String): Boolean;
var
  Code: Integer;
begin
  Result := Exec(ExpandConstant('{cmd}'),
                 '/C sc query "' + Svc + '" >nul 2>&1',
                 '', SW_HIDE, ewWaitUntilTerminated, Code) and (Code = 0);
end;

function ServiceIsStopped(Svc: String): Boolean;
var
  Code: Integer;
begin
  Result := Exec(ExpandConstant('{cmd}'),
                 '/C sc query "' + Svc + '" | findstr /C:"STATE" '
                 + '| findstr /C:"STOPPED"',
                 '', SW_HIDE, ewWaitUntilTerminated, Code) and (Code = 0);
end;

function GetDefaultDir(Param: String): String;
begin
  if IsUpgrade and (ExistingDir <> '') then
  begin
    Result := ExistingDir;
    exit;
  end;
  Result := 'C:\{#MyRepoName}';
  if GetSuffix() <> '' then
    Result := Result + '-' + GetSuffix();
end;

function GetAppUrl(Param: String): String;
begin
  Result := 'http://localhost:' + Trim(PortPage.Values[0]) + '/';
end;

// ---------- 后缀合法性 ----------
function SuffixIsValid(S: String): Boolean;
var
  I: Integer;
  C: Char;
begin
  Result := True;
  for I := 1 to Length(S) do
  begin
    C := S[I];
    if not (((C >= 'a') and (C <= 'z')) or ((C >= '0') and (C <= '9'))
            or (C = '-')) then
    begin
      Result := False;
      exit;
    end;
  end;
end;

// ---------- 升级检测 ----------
function UninstallRegKey(): String;
begin
  Result := 'SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\'
            + GetAppId('') + '_is1';
end;

procedure DetectUpgrade();
begin
  IsUpgrade := False;
  ExistingDir := '';
  if RegQueryStringValue(HKLM, UninstallRegKey(), 'InstallLocation',
                         ExistingDir) then
  begin
    ExistingDir := RemoveBackslash(ExistingDir);
    IsUpgrade := ExistingDir <> '';
  end;
end;

// ---------- 从现有 config.toml 预填 ----------
// TOML 基本字符串的反转义。必须与 TomlEscape 严格互为逆运算,否则升级预填
// 会悄悄改写含 \ 或 " 的密码(读回来不还原、写出去再转义一次 = 密码变了)。
// 顺序关键:先还原 \" 再还原 \\,反过来会把 \\" 错解成 \ + " 。
function TomlUnescape(S: String): String;
begin
  StringChangeEx(S, '\"', '"', True);
  StringChangeEx(S, '\\', '\', True);
  Result := S;
end;

function ReadTomlValue(Lines: TArrayOfString; Section, Key: String): String;
var
  I: Integer;
  Cur, L, K, V: String;
  P: Integer;
begin
  Result := '';
  Cur := '';
  for I := 0 to GetArrayLength(Lines) - 1 do
  begin
    L := Trim(Lines[I]);
    if (L <> '') and (L[1] = '[') then
      Cur := Copy(L, 2, Pos(']', L) - 2)
    else if (L <> '') and (L[1] <> '#') and (Cur = Section) then
    begin
      P := Pos('=', L);
      if P > 0 then
      begin
        K := Trim(Copy(L, 1, P - 1));
        if K = Key then
        begin
          V := Trim(Copy(L, P + 1, Length(L)));
          if (Length(V) >= 2) and (V[1] = '"') then
            V := TomlUnescape(Copy(V, 2, Length(V) - 2));
          Result := V;
          exit;
        end;
      end;
    end;
  end;
end;

procedure PrefillFromExisting();
var
  Lines: TArrayOfString;
  Path, V: String;
begin
  Path := ExistingDir + '\config.toml';
  if not FileExists(Path) then exit;
  if not LoadStringsFromFile(Path, Lines) then exit;

  V := ReadTomlValue(Lines, 'server', 'port');
  if V <> '' then PortPage.Values[0] := V;

  V := ReadTomlValue(Lines, 'db', 'db_type');
  if V <> '' then DbTypeCombo.Text := V;
  DbPage.Values[0] := ReadTomlValue(Lines, 'db', 'url');
  DbPage.Values[1] := ReadTomlValue(Lines, 'db', 'port');
  DbPage.Values[2] := ReadTomlValue(Lines, 'db', 'db_name');
  DbPage.Values[3] := ReadTomlValue(Lines, 'db', 'username');
  DbPage.Values[4] := ReadTomlValue(Lines, 'db', 'password');
  DbPage.Values[5] := ReadTomlValue(Lines, 'db', 'schema');

  RedisPage.Values[0] := ReadTomlValue(Lines, 'redis', 'url');
  RedisPage.Values[1] := ReadTomlValue(Lines, 'redis', 'port');
  RedisPage.Values[2] := ReadTomlValue(Lines, 'redis', 'password');
  RedisPage.Values[3] := ReadTomlValue(Lines, 'redis', 'db');
end;

// 按所选库类型刷新字段可用性与说明文案。
//
// 挂在 CurPageChanged 与 DbTypeCombo.OnChange 两处:此前只在进页时算一次,
// 用户在页内改类型后字段不刷新(选了 sqlite 地址端口仍可编辑)。
procedure ApplyDbTypeUi();
var
  T: String;
begin
  T := DbTypeCombo.Text;
  // 地址 / 端口:sqlite 不用;dm 的 SERVER 与 TCP_PORT 由 ODBC 数据源提供
  DbPage.Edits[0].Enabled := (T <> 'sqlite') and (T <> 'dm');
  DbPage.Edits[1].Enabled := (T <> 'sqlite') and (T <> 'dm');
  // 用户名 / 密码:sqlite 不用
  DbPage.Edits[3].Enabled := T <> 'sqlite';
  DbPage.Edits[4].Enabled := T <> 'sqlite';
  // schema:只有达梦与 PostgreSQL 认
  DbPage.Edits[5].Enabled := (T = 'dm') or (T = 'postgres');

  if T = 'dm' then
    DbHelpLabel.Caption :=
      '达梦:「库名」填已注册的 ODBC 数据源名(不是库名)。驱动与数据源需事先在'
      + '「ODBC 数据源管理器(64 位)」里配好,建表脚本见 sql/dm/。'#13#10
      + 'schema 即建表账号名(如 RUOYI),用建表账号登录时可留空。'
  else if T = 'postgres' then
    DbHelpLabel.Caption := 'schema 等价于 search_path,可留空。'
  else if T = 'sqlite' then
    DbHelpLabel.Caption := 'sqlite:「库名」填数据库文件路径,其余字段无效。'
  else
    DbHelpLabel.Caption := '';
end;

procedure DbTypeComboChange(Sender: TObject);
begin
  ApplyDbTypeUi();
end;

// ---------- 向导页 ----------
procedure InitializeWizard();
begin
  SuffixPage := CreateInputQueryPage(wpWelcome,
    '实例后缀', '同一台机器上区分多个实例',
    '正式环境请留空。测试环境为每个客户装一套时,填客户标识'
    + '(只允许小写字母、数字、连字符)。'#13#10#13#10
    + '后缀决定服务名与安装目录,装好后不可更改。');
  SuffixPage.Add('后缀:', False);

  PortPage := CreateInputQueryPage(wpSelectDir,
    '服务端口', '{#MyAppName} 后端监听的端口',
    '同机装多个实例时,各实例端口必须错开。');
  PortPage.Add('端口:', False);
  PortPage.Values[0] := '3000';

  DbPage := CreateInputQueryPage(PortPage.ID,
    '数据库', '请填一个已存在的空库,以及对它有读写权限的账号',
    '');
  DbPage.Add('地址:', False);
  DbPage.Add('端口:', False);
  DbPage.Add('库名:', False);
  DbPage.Add('用户名:', False);
  DbPage.Add('密码:', True);
  DbPage.Add('schema(达梦 / PostgreSQL,可留空):', False);
  DbPage.Values[0] := '127.0.0.1';
  DbPage.Values[1] := '3306';
  DbPage.Values[2] := 'solidaxum';
  DbPage.Values[3] := 'root';

  DbTypeCombo := TNewComboBox.Create(DbPage);
  DbTypeCombo.Parent := DbPage.Surface;
  DbTypeCombo.Style := csDropDownList;
  DbTypeCombo.Items.Add('mysql');
  DbTypeCombo.Items.Add('postgres');
  DbTypeCombo.Items.Add('sqlite');
  DbTypeCombo.Items.Add('dm');
  DbTypeCombo.ItemIndex := 0;
  DbTypeCombo.Top := DbPage.Edits[0].Top - ScaleY(28);
  DbTypeCombo.Left := DbPage.Edits[0].Left;
  DbTypeCombo.Width := DbPage.Edits[0].Width;
  DbTypeCombo.OnChange := @DbTypeComboChange;

  DbHelpLabel := TNewStaticText.Create(DbPage);
  DbHelpLabel.Parent := DbPage.Surface;
  DbHelpLabel.AutoSize := False;
  DbHelpLabel.WordWrap := True;
  DbHelpLabel.Left := DbPage.Edits[5].Left;
  DbHelpLabel.Top := DbPage.Edits[5].Top + ScaleY(24);
  DbHelpLabel.Width := DbPage.Edits[5].Width;
  DbHelpLabel.Height := ScaleY(52);

  RedisPage := CreateInputQueryPage(DbPage.ID,
    'Redis', '{#MyAppName} 使用的 Redis 实例',
    '点「下一步」时会实际连一次数据库和 Redis,连不上会停在本页。');
  RedisPage.Add('地址:', False);
  RedisPage.Add('端口:', False);
  RedisPage.Add('密码(无则留空):', True);
  RedisPage.Add('库编号:', False);
  RedisPage.Values[0] := 'localhost';
  RedisPage.Values[1] := '6379';
  RedisPage.Values[3] := '10';
end;

procedure CurPageChanged(CurPageID: Integer);
begin
  if CurPageID = DbPage.ID then
    ApplyDbTypeUi();
end;

// ---------- answers.toml ----------
function TomlEscape(S: String): String;
begin
  StringChangeEx(S, '\', '\\', True);
  StringChangeEx(S, '"', '\"', True);
  Result := S;
end;

function Q(S: String): String;
begin
  Result := '"' + TomlEscape(S) + '"';
end;

// 往 TArrayOfString 末尾追加一行。TArrayOfString 没有原生 push,这是标准惯用法。
procedure PushStr(var A: TArrayOfString; S: String);
begin
  SetArrayLength(A, GetArrayLength(A) + 1);
  A[GetArrayLength(A) - 1] := S;
end;

procedure WriteAnswers(Path: String);
var
  L: TArrayOfString;
begin
  SetArrayLength(L, 0);
  PushStr(L, '[server]');
  PushStr(L, 'port = ' + Trim(PortPage.Values[0]));
  PushStr(L, '');
  PushStr(L, '[db]');
  PushStr(L, 'db_type = ' + Q(DbTypeCombo.Text));
  PushStr(L, 'url = ' + Q(Trim(DbPage.Values[0])));
  PushStr(L, 'port = ' + Trim(DbPage.Values[1]));
  PushStr(L, 'db_name = ' + Q(Trim(DbPage.Values[2])));
  PushStr(L, 'username = ' + Q(Trim(DbPage.Values[3])));
  PushStr(L, 'password = ' + Q(DbPage.Values[4]));
  PushStr(L, 'schema = ' + Q(Trim(DbPage.Values[5])));
  PushStr(L, '');
  PushStr(L, '[redis]');
  PushStr(L, 'url = ' + Q(Trim(RedisPage.Values[0])));
  PushStr(L, 'port = ' + Trim(RedisPage.Values[1]));
  PushStr(L, 'password = ' + Q(RedisPage.Values[2]));
  PushStr(L, 'db = ' + Trim(RedisPage.Values[3]));
  PushStr(L, '');
  SaveStringsToFile(Path, L, False);
end;

// 按库类型把无意义的字段归一。answers 里 port 必须是合法整数,而被禁用的
// 输入框可能是空的 —— 这是本过程存在的硬理由。
procedure NormalizeDbFields();
var
  T: String;
begin
  T := DbTypeCombo.Text;
  // sqlite 与 dm 都不用地址 / 端口(dm 的 SERVER 由 ODBC 数据源提供)
  if (T = 'sqlite') or (T = 'dm') then
  begin
    DbPage.Values[0] := '';
    DbPage.Values[1] := '0';
  end;
  if T = 'sqlite' then
  begin
    DbPage.Values[3] := '';
    DbPage.Values[4] := '';
  end;
  // schema 只有 dm 与 postgres 认;切换库类型后必须清掉,否则残留语义错乱
  if (T <> 'dm') and (T <> 'postgres') then
    DbPage.Values[5] := '';
end;

// ---------- 连通性检查 ----------
function RunCheck(var ErrText: String): Boolean;
var
  Exe, Cfg, Ans, Log: String;
  Code: Integer;
  Lines: TArrayOfString;
  I: Integer;
begin
  Result := False;
  ErrText := '';

  // 只需提取 exe:Windows 的 odbc32.dll 是系统组件,无需随包附带
  ExtractTemporaryFile('{#MyRepoName}.exe');
  Exe := ExpandConstant('{tmp}\{#MyRepoName}.exe');
  Cfg := ExpandConstant('{tmp}\config.toml');
  Ans := ExpandConstant('{tmp}\answers.toml');
  Log := ExpandConstant('{tmp}\check.log');

  NormalizeDbFields();
  WriteAnswers(Ans);

  // 升级:以现有 config.toml 为起点,write-config 只做外科更新,
  // security.aes_key / jwt.secret 与手改的高级项因此得以保留。
  if IsUpgrade and FileExists(ExistingDir + '\config.toml') then
  begin
    if not CopyFile(ExistingDir + '\config.toml', Cfg, False) then
    begin
      ErrText := '无法读取现有配置: ' + ExistingDir + '\config.toml';
      exit;
    end;
  end
  else
    DeleteFile(Cfg);

  if not Exec(Exe, '--write-config "' + Cfg + '" --answers "' + Ans + '"',
              ExpandConstant('{tmp}'), SW_HIDE, ewWaitUntilTerminated, Code)
     or (Code <> 0) then
  begin
    ErrText := '生成配置失败(退出码 ' + IntToStr(Code) + ')';
    exit;
  end;

  // Exec 拿不到 stderr,用 cmd 重定向到文件再读回来
  if not Exec(ExpandConstant('{cmd}'),
              '/C ""' + Exe + '" --check-config "' + Cfg + '" 2> "' + Log + '""',
              ExpandConstant('{tmp}'), SW_HIDE, ewWaitUntilTerminated, Code) then
  begin
    ErrText := '无法启动配置检查程序';
    exit;
  end;

  if Code = 0 then
  begin
    Result := True;
    exit;
  end;

  if LoadStringsFromFile(Log, Lines) then
    for I := 0 to GetArrayLength(Lines) - 1 do
      ErrText := ErrText + Lines[I] + #13#10;
  if Trim(ErrText) = '' then
    ErrText := '连接检查失败(退出码 ' + IntToStr(Code) + ')';
end;

// ---------- 端口占用探测 ----------
function PortInUse(Port: Integer): Boolean;
var
  Code: Integer;
begin
  Result := False;
  if Exec(ExpandConstant('{cmd}'),
          '/C netstat -an -p TCP | findstr LISTENING | findstr ":' + IntToStr(Port) + ' "',
          '', SW_HIDE, ewWaitUntilTerminated, Code) and (Code = 0) then
    Result := True;
end;

function NextButtonClick(CurPageID: Integer): Boolean;
var
  Port, ErrText: String;
  N: Integer;
begin
  Result := True;

  if CurPageID = SuffixPage.ID then
  begin
    if not SuffixIsValid(GetSuffix()) then
    begin
      MsgBox('后缀只能包含小写字母、数字和连字符。', mbError, MB_OK);
      Result := False;
      exit;
    end;
    DetectUpgrade();
    // 非升级却已存在同名服务 = 这台机器用旧 zip + install.ps1 装过。
    if (not IsUpgrade) and ServiceExists(GetServiceName('')) then
    begin
      MsgBox('检测到系统中已存在服务 ' + GetServiceName('')
             + ',但它不是由本安装程序装的。'#13#10#13#10
             + '这台机器很可能用旧的 zip 包装过。请先在旧的安装目录里'
             + '以管理员身份运行 uninstall.ps1 注销该服务'
             + '(它只注销服务,不删文件),备份好 config.toml 后再重新安装。'
             + #13#10#13#10
             + '若要与现有实例并存,请返回上一页填一个不同的实例后缀。',
             mbError, MB_OK);
      Result := False;
      exit;
    end;
    WizardForm.DirEdit.Text := GetDefaultDir('');
  end

  else if CurPageID = wpSelectDir then
  begin
    if IsUpgrade then
      PrefillFromExisting();
  end

  else if CurPageID = PortPage.ID then
  begin
    Port := Trim(PortPage.Values[0]);
    N := StrToIntDef(Port, -1);
    if (N < 1) or (N > 65535) then
    begin
      MsgBox('端口必须是 1-65535 之间的数字。', mbError, MB_OK);
      Result := False;
      exit;
    end;
    if (not IsUpgrade) and PortInUse(N) then
    begin
      MsgBox('端口 ' + Port + ' 已被占用,请换一个(同机多实例必须错开)。'#13#10
             + '若刚卸载过旧实例,稍等几秒让端口释放后重试。',
             mbError, MB_OK);
      Result := False;
      exit;
    end;
  end

  else if CurPageID = DbPage.ID then
  begin
    if Trim(DbPage.Values[2]) = '' then
    begin
      if DbTypeCombo.Text = 'dm' then
        MsgBox('ODBC 数据源名不能为空。', mbError, MB_OK)
      else if DbTypeCombo.Text = 'sqlite' then
        MsgBox('数据库文件路径不能为空。', mbError, MB_OK)
      else
        MsgBox('库名不能为空。', mbError, MB_OK);
      Result := False;
      exit;
    end;
    // 端口:dm 与 sqlite 不用它(NormalizeDbFields 会把它刷成 0),其余类型
    // 必须校验 —— 从 dm/sqlite 切回来时留下的 0 否则会被静默带进 answers,
    // 拼出 `mysql://…:0/db` 这种连不上的 DSN。Linux 侧由 ask 的 port_is_valid
    // 一直重问兜住,Windows 侧没有那道闸,只能在这里拦。
    if (DbTypeCombo.Text <> 'dm') and (DbTypeCombo.Text <> 'sqlite') then
    begin
      N := StrToIntDef(Trim(DbPage.Values[1]), -1);
      if (N < 1) or (N > 65535) then
      begin
        MsgBox('数据库端口必须是 1-65535 之间的数字。', mbError, MB_OK);
        Result := False;
        exit;
      end;
    end;
    // 双引号会截断 SET SCHEMA 的标识符,后端也会拒;只在认 schema 的类型下
    // 校验 —— 字段已禁用的类型不拦,否则残留值会把翻页卡死
    if ((DbTypeCombo.Text = 'dm') or (DbTypeCombo.Text = 'postgres'))
      and (Pos('"', DbPage.Values[5]) > 0) then
    begin
      MsgBox('schema 名不能包含双引号。', mbError, MB_OK);
      Result := False;
      exit;
    end;
  end

  else if CurPageID = RedisPage.ID then
  begin
    WizardForm.Repaint();
    if not RunCheck(ErrText) then
    begin
      MsgBox('配置检查未通过,请修正后重试:'#13#10#13#10 + ErrText,
             mbError, MB_OK);
      Result := False;
    end;
  end;
end;

// ---------- 升级前停服务 ----------
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Code, I: Integer;
  SvcExe, SvcName: String;
begin
  Result := '';
  NeedsRestart := False;
  if not IsUpgrade then exit;

  SvcName := GetServiceName('');
  if not ServiceExists(SvcName) then exit;

  SvcExe := ExistingDir + '\service\' + SvcName + '.exe';
  if FileExists(SvcExe) then
    Exec(SvcExe, 'stop', ExistingDir, SW_HIDE, ewWaitUntilTerminated, Code)
  else
    Exec(ExpandConstant('{cmd}'), '/C sc stop "' + SvcName + '" >nul 2>&1',
         '', SW_HIDE, ewWaitUntilTerminated, Code);

  // WinSW stop 返回时服务主进程未必已退出,若立刻覆盖 exe 会撞上文件占用。
  // 轮询服务状态直到 STOPPED。
  for I := 1 to 30 do
  begin
    if ServiceIsStopped(SvcName) then exit;
    Sleep(1000);
  end;

  Result := '服务 ' + SvcName + ' 在 30 秒内没有停止,无法覆盖程序文件。'#13#10
            + '请手工执行 Stop-Service ' + SvcName + ' 后重新运行安装程序。';
end;

// ---------- 收尾 ----------
function RenderWinswXml(): Boolean;
var
  Lines: TArrayOfString;
  I: Integer;
  S: String;
begin
  Result := False;
  if not LoadStringsFromFile(
       ExpandConstant('{app}\templates\winsw\{#MyRepoName}.xml'), Lines) then
    exit;
  for I := 0 to GetArrayLength(Lines) - 1 do
  begin
    S := Lines[I];
    StringChangeEx(S, '@PREFIX@', ExpandConstant('{app}'), True);
    StringChangeEx(S, '@SVC_NAME@', GetServiceName(''), True);
    StringChangeEx(S, '@REPO_NAME@', '{#MyRepoName}', True);
    Lines[I] := S;
  end;
  Result := SaveStringsToFile(
    ExpandConstant('{app}\service\') + GetServiceName('') + '.xml',
    Lines, False);
end;

function PortIsOpen(): Boolean;
var
  Code: Integer;
begin
  // 用后端自己的 /api/health 探活比探 TCP 更准:端口开着不代表路由就绪
  Result := Exec(ExpandConstant('{cmd}'),
    '/C curl -s -o nul --max-time 2 http://localhost:'
    + Trim(PortPage.Values[0]) + '/api/health',
    '', SW_HIDE, ewWaitUntilTerminated, Code) and (Code = 0);
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Code, I: Integer;
  Svc, SvcName: String;
  NameLine: TArrayOfString;
begin
  if CurStep <> ssPostInstall then exit;

  SvcName := GetServiceName('');

  // 1. 落配置(测通的那一份原样搬过来)
  if not CopyFile(ExpandConstant('{tmp}\config.toml'),
                  ExpandConstant('{app}\config.toml'), False) then
  begin
    RaiseException('写入 config.toml 失败: ' + ExpandConstant('{app}')
                   + '\config.toml');
  end;

  // 2. 收紧权限:里面有数据库密码和加密密钥
  Exec(ExpandConstant('{sys}\icacls.exe'),
       '"' + ExpandConstant('{app}\config.toml') + '" /inheritance:r'
       + ' /grant:r "*S-1-5-18:F" /grant:r "*S-1-5-32-544:F"',
       '', SW_HIDE, ewWaitUntilTerminated, Code);

  // 3. 运行目录
  CreateDir(ExpandConstant('{app}\data'));
  CreateDir(ExpandConstant('{app}\logs'));
  CreateDir(ExpandConstant('{app}\service'));

  // 4. 落服务名,供卸载程序读回(那时 SuffixPage 为 nil,推导不出后缀)
  SetArrayLength(NameLine, 1);
  NameLine[0] := SvcName;
  if not SaveStringsToFile(ServiceNameFile(), NameLine, False) then
    RaiseException('写入服务名标记失败: ' + ServiceNameFile());

  // 5. WinSW
  if not RenderWinswXml() then
    RaiseException('渲染 WinSW 服务配置失败,请确认 '
                   + ExpandConstant('{app}\templates\winsw\{#MyRepoName}.xml')
                   + ' 存在。');

  Svc := ExpandConstant('{app}\service\') + SvcName + '.exe';
  if not CopyFile(ExpandConstant('{app}\WinSW-x64.exe'), Svc, False) then
    RaiseException('复制服务 wrapper 失败: ' + Svc);

  // 6. 注册。升级时服务已存在,WinSW install 会失败,那是预期的;
  //    全新安装时失败就是真失败,必须报错,不能装到一半装作没事。
  if not Exec(Svc, 'install', ExpandConstant('{app}'), SW_HIDE,
              ewWaitUntilTerminated, Code) then
    RaiseException('无法启动服务注册程序: ' + Svc);
  if (Code <> 0) and (not ServiceExists(SvcName)) then
    RaiseException('注册 Windows 服务失败(WinSW 退出码 ' + IntToStr(Code)
                   + '),服务 ' + SvcName + ' 未创建。');

  // 7. 启动
  if not Exec(Svc, 'start', ExpandConstant('{app}'), SW_HIDE,
              ewWaitUntilTerminated, Code) then
    RaiseException('无法启动服务启动程序: ' + Svc);

  // 8. 探活,最多 30 秒
  for I := 1 to 30 do
  begin
    if PortIsOpen() then
    begin
      Exec(ExpandConstant('{cmd}'), '/C start "" "' + GetAppUrl('') + '"', '',
           SW_HIDE, ewNoWait, Code);
      exit;
    end;
    Sleep(1000);
  end;

  // 走到这里:服务确实注册上了(第 6 步已验证),只是没在 30 秒内起来。
  MsgBox('服务 ' + SvcName + ' 已注册,但 30 秒内未响应健康检查。'#13#10#13#10
         + '配置已保存到 ' + ExpandConstant('{app}\config.toml') + '。'#13#10
         + '请查看 ' + ExpandConstant('{app}\logs') + ' 排查后执行:'#13#10
         + '  Start-Service ' + SvcName,
         mbInformation, MB_OK);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    MsgBox('程序已卸载。'#13#10#13#10
           + '配置与数据保留在:'#13#10
           + ExpandConstant('{app}') + #13#10 + #13#10
           + '(config.toml / data / logs)。需要彻底清理请手工删除该文件夹。',
           mbInformation, MB_OK);
end;
