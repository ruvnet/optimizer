; RuVector MemOpt Installer Script (Inno Setup)

#define MyAppName "RuVector Memory Optimizer"
#define MyAppVersion "0.2.2"
#define MyAppPublisher "ruv"
#define MyAppURL "https://github.com/ruvnet/ruvector-memopt"
#define MyAppExeName "ruvector-memopt.exe"

[Setup]
AppId={{8A7B3C9D-4E5F-6A7B-8C9D-0E1F2A3B4C5D}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
DefaultDirName={autopf}\RuVectorMemOpt
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
OutputDir=..\dist
OutputBaseFilename=RuVectorMemOpt-Setup
SetupIconFile=..\assets\icon.ico
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "startuptask"; Description: "Auto-optimize memory at startup (Recommended)"; GroupDescription: "Startup:"; Flags: checkedonce

[Files]
Source: "..\target\release\ruvector-memopt.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Parameters: "tray"
Name: "{group}\Memory Status"; Filename: "{app}\{#MyAppExeName}"; Parameters: "status"
Name: "{group}\Optimize Now"; Filename: "{app}\{#MyAppExeName}"; Parameters: "optimize"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Parameters: "tray"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "RuVectorMemOpt"; ValueData: """{app}\{#MyAppExeName}"" tray"; Flags: uninsdeletevalue; Tasks: startuptask

[Run]
Filename: "{app}\{#MyAppExeName}"; Parameters: "tray"; Description: "Start RuVector Memory Optimizer"; Flags: nowait postinstall skipifsilent
