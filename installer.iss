; Task Cleaner Windows 11 Fluent Setup Script
; Tool: Inno Setup 6.x
; Dual-licensed under GNU AGPLv3 and Commercial License.

#ifndef MyAppVersion
#define MyAppVersion "1.0.0"
#endif

#define MyAppName "Task Cleaner"
#define MyAppPublisher "DonJone"
#define MyAppURL "https://github.com/macos-task-cleaner/windows-task-cleaner-gui"
#define MyAppExeName "TaskCleaner.exe"
#define MyCliExeName "mtc.exe"

[Setup]
AppId={{D37E580B-4D2A-4B7C-A59B-1B2F995C7221}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={localappdata}\Programs\TaskCleaner
DisableProgramGroupPage=yes

; 采用当前用户权限安装，无需 UAC 弹窗提权
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog

OutputDir=dist
OutputBaseFilename=TaskCleaner-Windows-x64-Setup
SetupIconFile=app.ico
UninstallIconFile=app.ico
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes

[Languages]
Name: "chinesesimplified"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "startupicon"; Description: "开机自动启动 Task Cleaner (推荐)"; GroupDescription: "系统集成"; Flags: checked
Name: "addtopath"; Description: "将 mtc 命令行工具添加至用户 PATH 环境变量"; GroupDescription: "命令行集成"; Flags: checked

[Files]
Source: "publish\x64\TaskCleaner.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "publish\x64\mtc.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.md"; DestDir: "{app}"; Flags: ignoreversion isreadme
Source: "LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "COMMERCIAL.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "assets\app.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\app.ico"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\app.ico"; Tasks: desktopicon
Name: "{userstartup}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\app.ico"; Tasks: startupicon

[Registry]
; 注册 PATH 环境变量支持
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Tasks: addtopath; Check: NeedsAddPath(ExpandConstant('{app}'))

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: files; Name: "{app}\app.ico"

[Code]
// 检测 PATH 是否已包含该目录，避免重复追加
function NeedsAddPath(Param: string): boolean;
var
  OrigPath: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath) then
  begin
    Result := True;
    exit;
  end;
  Result := Pos(';' + Param + ';', ';' + OrigPath + ';') = 0;
end;
