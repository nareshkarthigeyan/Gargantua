#ifndef AppVersion
  #define AppVersion "0.2.0"
#endif
[Setup]
AppId={{B68C201C-CA46-41A5-907C-0D9827E849BC}
AppName=Gargantua
AppVersion={#AppVersion}
AppPublisher=nareshkarthigeyan
AppPublisherURL=https://github.com/nareshkarthigeyan/Gargantua
AppSupportURL=https://github.com/nareshkarthigeyan/Gargantua/issues
DefaultDirName={localappdata}\Programs\Gargantua
DefaultGroupName=Gargantua
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
OutputDir=..\..\dist
OutputBaseFilename=Gargantua-{#AppVersion}-windows-x64-setup
SetupIconFile=..\..\assets\icons\gargantua.ico
UninstallDisplayIcon={app}\Gargantua.exe
LicenseFile=..\..\LICENSE
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
[Files]
Source: "..\..\target\release\gargantua.exe"; DestDir: "{app}"; DestName: "Gargantua.exe"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"
Source: "..\..\README.md"; DestDir: "{app}"
Source: "..\..\assets\fonts\OFL.txt"; DestDir: "{app}\licenses"
[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked
[Icons]
Name: "{group}\Gargantua"; Filename: "{app}\Gargantua.exe"
Name: "{autodesktop}\Gargantua"; Filename: "{app}\Gargantua.exe"; Tasks: desktopicon
[Run]
Filename: "{app}\Gargantua.exe"; Description: "Launch Gargantua"; Flags: nowait postinstall skipifsilent
