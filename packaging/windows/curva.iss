; Inno Setup script for the Windows installer. The release workflow passes
; /DVersion=<x.y.z> and /DSource=<folder holding curva.exe and the licence files>.
; It installs for the current user only, so no administrator rights are needed.

#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef Source
  #define Source "..\..\dist\windows"
#endif

[Setup]
AppId={{6F1C2D4E-8B3A-4C5D-9E7F-0A1B2C3D4E5F}
AppName=Curva
AppVersion={#Version}
AppPublisher=The Curva contributors
AppPublisherURL=https://github.com/curva3d/curva
DefaultDirName={autopf}\Curva
DefaultGroupName=Curva
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputBaseFilename=curva-v{#Version}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; The installer itself shows Curva's icon; the program carries it too.
SetupIconFile=..\icons\curva.ico
UninstallDisplayIcon={app}\curva.exe

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "{#Source}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{group}\Curva"; Filename: "{app}\curva.exe"
Name: "{group}\Uninstall Curva"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Curva"; Filename: "{app}\curva.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\curva.exe"; Description: "{cm:LaunchProgram,Curva}"; Flags: nowait postinstall skipifsilent
