; Sevi for Windows installer (Inno Setup 6). Per-user, no admin prompt, so the app can update itself.
;   iscc /DVersion=1.0.14 /DArch=x64 /DSrc=..\build\App Sevi.iss
#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef Arch
  #define Arch "x64"
#endif
#ifndef Src
  #define Src "..\build\App"
#endif

[Setup]
AppId={{7E2B5C1A-9D3F-4C7B-8E61-SEVI0000001}
AppName=Sevi
AppVersion={#Version}
AppVerName=Sevi {#Version}
AppPublisher=Sevi contributors
AppPublisherURL=https://github.com/tuanxtm/sevi
AppSupportURL=https://github.com/tuanxtm/sevi/issues
AppUpdatesURL=https://github.com/tuanxtm/sevi/releases/latest
VersionInfoVersion={#Version}
DefaultDirName={localappdata}\Programs\Sevi
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir=.\out
OutputBaseFilename=Sevi-{#Version}-{#Arch}-Setup
SetupIconFile=..\App\Sevi.ico
UninstallDisplayIcon={app}\Sevi.exe
UninstallDisplayName=Sevi
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
CloseApplicationsFilter=Sevi.exe
RestartApplications=no
#if Arch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"

[Messages]
WelcomeLabel1=Install Sevi
WelcomeLabel2=Sevi {#Version} will be installed for your account.%n%nClick Next to continue.
FinishedHeadingLabel=Sevi is installed
FinishedLabel=Sevi runs in the notification area. Press Alt+Z to switch between English and Vietnamese.
ConfirmUninstall=Uninstall Sevi?

[Files]
Source: "{#Src}\Sevi.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Src}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Src}\THIRD_PARTY_NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Sevi"; Filename: "{app}\Sevi.exe"

[Registry]
; the app's own "Mở Sevi khi đăng nhập" entry goes away with the app
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "Sevi"; Flags: uninsdeletevalue dontcreatekey

[Run]
Filename: "{app}\Sevi.exe"; Description: "Launch Sevi"; Flags: nowait postinstall skipifsilent
; a self-update (Sevi runs the installer silently with /RELAUNCH=1) brings the app back
Filename: "{app}\Sevi.exe"; Flags: nowait; Check: ShouldRelaunch

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/IM Sevi.exe /F"; Flags: runhidden; RunOnceId: "KillSevi"

[Code]
function ShouldRelaunch: Boolean;
begin
  Result := ExpandConstant('{param:RELAUNCH|0}') = '1';
end;
