; Task Cleaner Windows 11 Fluent Setup Script
; Tool: Inno Setup 6.x
; Dual-licensed under GNU AGPLv3 and Commercial License.

#ifndef MyAppVersion
#define MyAppVersion "1.0.0"
#endif

#ifndef AppArch
#define AppArch "x64"
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

; 采用当前用户权限安装，无需 UAC 提权
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog

OutputDir=dist
OutputBaseFilename=TaskCleaner-Windows-{#AppArch}-Setup
SetupIconFile=app.ico
UninstallDisplayIcon={app}\app.ico
UninstallDisplayName={#MyAppName}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern

#if AppArch == "x64"
ArchitecturesInstallIn64BitMode=x64compatible
ArchitecturesAllowed=x64compatible
#elif AppArch == "arm64"
ArchitecturesInstallIn64BitMode=arm64
ArchitecturesAllowed=arm64
#endif

CloseApplications=yes
CloseApplicationsFilter=TaskCleaner.exe,mtc.exe
AppMutex=TaskCleaner_Win32_SingleInstance_Mutex_2026
ChangesEnvironment=yes

VersionInfoVersion={#MyAppVersion}
VersionInfoProductVersion={#MyAppVersion}
VersionInfoCompany={#MyAppPublisher}
VersionInfoDescription={#MyAppName} Setup
VersionInfoCopyright=Copyright (c) 2026 DonJone

[Languages]
Name: "chinesesimplified"; MessagesFile: "assets\languages\ChineseSimplified.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "startupicon"; Description: "开机自动启动 Task Cleaner"; GroupDescription: "系统集成"
Name: "addtopath"; Description: "将 mtc 命令行工具添加至用户 PATH 环境变量"; GroupDescription: "环境配置"

[Files]
Source: "publish\{#AppArch}\TaskCleaner.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "publish\{#AppArch}\mtc.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.md"; DestDir: "{app}"; Flags: ignoreversion isreadme
Source: "LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "COMMERCIAL.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "assets\app.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\app.ico"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\app.ico"; Tasks: desktopicon
Name: "{userstartup}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\app.ico"; Tasks: startupicon

[Registry]
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Tasks: addtopath; Check: NeedsAddPath(ExpandConstant('{app}'))

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: files; Name: "{app}\app.ico"

[Code]
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

procedure RemovePath(Param: string);
var
  OrigPath, NewPath: string;
  P: Integer;
begin
  if RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath) then
  begin
    NewPath := OrigPath;
    P := Pos(Param + ';', NewPath);
    if P > 0 then
      Delete(NewPath, P, Length(Param) + 1)
    else
    begin
      P := Pos(';' + Param, NewPath);
      if P > 0 then
        Delete(NewPath, P, Length(Param) + 1)
      else if CompareText(NewPath, Param) = 0 then
        NewPath := '';
    end;
    RegWriteStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', NewPath);
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  AppDataDir: string;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    RemovePath(ExpandConstant('{app}'));
    AppDataDir := ExpandConstant('{userappdata}\TaskCleaner');
    if DirExists(AppDataDir) then
    begin
      if MsgBox('是否同时删除保存于 AppData 的用户配置文件和白名单缓存？', mbConfirmation, MB_YESNO) = idYes then
      begin
        DelTree(AppDataDir, True, True, True);
      end;
    end;
  end;
end;
