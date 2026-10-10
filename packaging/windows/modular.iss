; Modular's Windows installer, built by release.yml with Inno Setup 6:
;
;   iscc /DVersion=0.4.0 /DFlavor=standard /DSourceDir=<release folder> /DOutputDir=<out> packaging\windows\modular.iss
;
; It installs for the current user only, into %LOCALAPPDATA%\Programs\Modular,
; so there's no admin prompt and the in-app updater can write there. It adds
; a Start Menu shortcut and an uninstaller in Apps & Features.
;
; Both downloads share one AppId: installing the ASIO one over the standard
; one (or back) switches the copy in place rather than adding a second app.

#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef Flavor
  #define Flavor "standard"
#endif
#ifndef SourceDir
  #define SourceDir "..\..\target\release"
#endif
#ifndef OutputDir
  #define OutputDir "..\..\target\installer"
#endif

#if Flavor == "asio"
  #define OutputName "modular_synth-windows-asio-setup"
  #define FlavorName " (ASIO)"
#else
  #define OutputName "modular_synth-windows-setup"
  #define FlavorName ""
#endif

[Setup]
AppId={{9C3F6E1A-4B7D-4E58-9A62-6D0E2B71C5F4}
AppName=Modular Synth
AppVersion={#Version}
AppVerName=Modular Synth {#Version}{#FlavorName}
AppPublisher=Chris Chappelear
AppPublisherURL=https://github.com/chrischaps/Modular
AppSupportURL=https://docs.chaps.dev/modular/
AppUpdatesURL=https://github.com/chrischaps/Modular/releases
; {autopf} is %LOCALAPPDATA%\Programs when installing for one user
PrivilegesRequired=lowest
DefaultDirName={autopf}\Modular
DisableProgramGroupPage=yes
UninstallDisplayName=Modular Synth{#FlavorName}
UninstallDisplayIcon={app}\modular_synth.exe
SetupIconFile=..\..\assets\icon\modular.ico
OutputDir={#OutputDir}
OutputBaseFilename={#OutputName}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; A running copy is closed first (the in-app updater is the way to update
; while it's open)
CloseApplications=yes
#if Flavor == "asio"
; The ASIO build is distributed under the GPLv3: say so before installing
LicenseFile={#SourceDir}\GPL-3.0.txt
InfoBeforeFile={#SourceDir}\README-LOW-LATENCY.txt
#else
LicenseFile={#SourceDir}\LICENSE
#endif

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\modular_synth.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
#if Flavor == "asio"
Source: "{#SourceDir}\README-LOW-LATENCY.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\GPL-3.0.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\ASIO-SDK-LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
#endif

[InstallDelete]
; Switching to the standard build takes the ASIO build's GPL papers with it
#if Flavor != "asio"
Type: files; Name: "{app}\README-LOW-LATENCY.txt"
Type: files; Name: "{app}\GPL-3.0.txt"
Type: files; Name: "{app}\ASIO-SDK-LICENSE.txt"
#endif

[Icons]
Name: "{autoprograms}\Modular Synth"; Filename: "{app}\modular_synth.exe"; Comment: "A node-based modular synthesizer"
Name: "{autodesktop}\Modular Synth"; Filename: "{app}\modular_synth.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\modular_synth.exe"; Description: "{cm:LaunchProgram,Modular Synth}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; What in-app updates leave beside the executable: the version kept to roll
; back to, an interrupted download, and any notes a later release brought
Type: files; Name: "{app}\modular_synth.old"
Type: files; Name: "{app}\modular_synth.old.version"
Type: files; Name: "{app}\modular_synth.rollback"
Type: filesandordirs; Name: "{app}\.modular-update"
Type: files; Name: "{app}\*.txt"
Type: dirifempty; Name: "{app}"
