[Setup]
AppName=eBIRForms
AppVersion={#MyAppVersion}
DefaultDirName={autopf}\eBIRForms
DefaultGroupName=eBIRForms
OutputDir=target\release-artifacts
OutputBaseFilename=eBIRForms-Windows-x64-{#MyAppVersion}-Setup
Compression=lzma2
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\bir.exe

[Files]
Source: "target\x86_64-pc-windows-msvc\release\bir.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "target\x86_64-pc-windows-msvc\release\bir-daemon.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "assets\*"; DestDir: "{app}\assets"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "formtypes\*"; DestDir: "{app}\formtypes"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\eBIRForms"; Filename: "{app}\bir.exe"
Name: "{autodesktop}\eBIRForms"; Filename: "{app}\bir.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop icon"; GroupDescription: "Additional icons:"
