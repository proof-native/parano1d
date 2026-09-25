#ifndef MyAppVersion
  #error MyAppVersion is required
#endif
#ifndef SourceDir
  #error SourceDir is required
#endif
#ifndef NumericVersion
  #error NumericVersion is required
#endif
#ifndef OutputDir
  #error OutputDir is required
#endif
#ifndef OutputBaseFilename
  #error OutputBaseFilename is required
#endif
#ifndef IconFile
  #error IconFile is required
#endif
#ifndef LicenseFile
  #error LicenseFile is required
#endif
#ifndef NoticeFile
  #error NoticeFile is required
#endif
#ifndef UserGuideFile
  #error UserGuideFile is required
#endif
#ifndef ContractGuideFile
  #error ContractGuideFile is required
#endif

[Setup]
AppId={{8EAD67A1-91AB-497A-81A5-8A73CF4A6F31}
AppName=Parano1d
AppVersion={#MyAppVersion}
AppVerName=Parano1d {#MyAppVersion}
AppPublisher=Paranoid Zero
AppPublisherURL=https://parano1d.org/
AppSupportURL=https://github.com/ignotusnemo/parano1d/issues
AppUpdatesURL=https://github.com/ignotusnemo/parano1d/releases
DefaultDirName={localappdata}\Programs\Parano1d
DefaultGroupName=Parano1d
DisableProgramGroupPage=yes
AllowNoIcons=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutputDir}
OutputBaseFilename={#OutputBaseFilename}
SetupIconFile={#IconFile}
LicenseFile={#LicenseFile}
UninstallDisplayIcon={app}\Parano1d.exe
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
SetupLogging=yes
VersionInfoVersion={#NumericVersion}
VersionInfoCompany=Paranoid Zero
VersionInfoDescription=Parano1d Wallet Installer
VersionInfoProductName=Parano1d
VersionInfoProductVersion={#MyAppVersion}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "chinesesimplified"; MessagesFile: "{#SourcePath}\ChineseSimplified.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\parano1d-gui.exe"; DestDir: "{app}"; DestName: "Parano1d.exe"; Flags: ignoreversion
Source: "{#SourceDir}\parano1d.exe"; DestDir: "{app}"; DestName: "parano1d-node.exe"; Flags: ignoreversion
Source: "{#LicenseFile}"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "{#NoticeFile}"; DestDir: "{app}"; DestName: "NOTICE.txt"; Flags: ignoreversion
Source: "{#UserGuideFile}"; DestDir: "{app}"; DestName: "README.txt"; Flags: ignoreversion
Source: "{#ContractGuideFile}"; DestDir: "{app}"; DestName: "CONTRACTS.md"; Flags: ignoreversion

[Icons]
Name: "{group}\Parano1d"; Filename: "{app}\Parano1d.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\Parano1d"; Filename: "{app}\Parano1d.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\Parano1d.exe"; Description: "{cm:LaunchProgram,Parano1d}"; Flags: nowait postinstall skipifsilent
