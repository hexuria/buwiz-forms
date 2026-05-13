[Setup]
AppName=eBIRForms
AppVersion={#MyAppVersion}
AppPublisher=GoldCoders Corp
AppPublisherURL=https://goldcoders.dev
AppSupportURL=https://github.com/codeitlikemiley/ebirforms/issues
AppUpdatesURL=https://github.com/codeitlikemiley/ebirforms/releases
DefaultDirName={autopf}\eBIRForms
DefaultGroupName=eBIRForms
OutputDir=target\release-artifacts
OutputBaseFilename=eBIRForms-Windows-x64-{#MyAppVersion}-Setup
Compression=lzma2
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\bir.exe
SetupIconFile=assets\icon.ico
WizardStyle=modern
; Sign the installer if a certificate is available (CI will pass /S flags)
; SignTool=signtool sign /f "$CERT_FILE" /p "$CERT_PASSWORD" /t http://timestamp.digicert.com $f

[Files]
Source: "target\x86_64-pc-windows-msvc\release\bir.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "assets\*"; DestDir: "{app}\assets"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "formtypes\*"; DestDir: "{app}\formtypes"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\eBIRForms"; Filename: "{app}\bir.exe"; IconFilename: "{app}\assets\icon.ico"
Name: "{autodesktop}\eBIRForms"; Filename: "{app}\bir.exe"; IconFilename: "{app}\assets\icon.ico"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop icon"; GroupDescription: "Additional icons:"

[Run]
Filename: "{win}\explorer.exe"; Parameters: """{app}\bir.exe"""; Description: "Launch eBIRForms"; Flags: nowait postinstall skipifsilent

