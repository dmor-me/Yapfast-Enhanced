#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef EnhancedVersion
  #error EnhancedVersion must be supplied by scripts/package-enhanced-inplace.py
#endif
#ifndef BundleDir
  #error BundleDir is required
#endif

[Setup]
AppId={{437563B0-0C9A-4B3B-978E-8D057910AB77}
AppName=Yapfast Enhanced
AppVersion={#EnhancedVersion}
VersionInfoVersion={#EnhancedVersion}
AppPublisher=Yapfast Enhanced
AppVerName=Yapfast Enhanced
SetupIconFile=branding/yapfast-enhanced.ico
DefaultDirName={localappdata}\Programs\Yapfast
UninstallFilesDir={app}\.yapfast-enhanced\uninstall
DefaultGroupName=Yapfast
DisableProgramGroupPage=yes
DisableDirPage=no
DisableReadyPage=no
DirExistsWarning=no
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputBaseFilename=Yapfast-Enhanced-Installer
#ifdef StylePreview
Compression=none
#else
Compression=lzma2
#endif
SolidCompression=yes
WizardStyle=modern dark hidebevels includetitlebar
WizardBackColor=#141518
WizardSizePercent=120,110
WizardImageFile=branding/sidebar.bmp
WizardSmallImageFile=branding/header.bmp
WizardImageStretch=yes
WizardImageBackColor=#141518
WizardSmallImageBackColor=#141518
WizardImageAlphaFormat=none
DefaultDialogFontName=Segoe UI
DisableWelcomePage=no
UninstallDisplayIcon={app}\.yapfast-enhanced\yapfast-enhanced.ico
LicenseFile={#BundleDir}\LICENSE.txt
ExtraDiskSpaceRequired=1073741824
CloseApplications=no
RestartApplications=no

; Cold copies stay available offline. Live executables are never compressed.
[Dirs]
Name: "{app}\.yapfast-enhanced\backups"; Flags: unsetntfscompression uninsneveruninstall

[Tasks]
Name: repairshortcut; Description: "Create a Yapfast Enhanced Repair shortcut"; GroupDescription: "Utilities:"; Flags: unchecked

[Files]
Source: "branding\yapfast-enhanced.ico"; DestDir: "{app}\.yapfast-enhanced"; Flags: ignoreversion
Source: "{#BundleDir}\manager.exe"; DestDir: "{app}\.yapfast-enhanced"; Flags: ignoreversion
Source: "{#BundleDir}\payload\manifest.json"; DestDir: "{app}\.yapfast-enhanced\payload"; Flags: ignoreversion
Source: "{#BundleDir}\payload\yapfast.exe"; DestDir: "{app}\.yapfast-enhanced\payload"; Flags: ignoreversion unsetntfscompression
Source: "{#BundleDir}\payload\yapfast-plugins.exe"; DestDir: "{app}\.yapfast-enhanced\payload"; Flags: ignoreversion unsetntfscompression
Source: "{#BundleDir}\LICENSE.txt"; DestDir: "{app}\.yapfast-enhanced"; Flags: ignoreversion
Source: "{#BundleDir}\SOURCE.txt"; DestDir: "{app}\.yapfast-enhanced"; Flags: ignoreversion
Source: "{#BundleDir}\examples\Ping\*"; DestDir: "{app}\.yapfast-enhanced\examples\Ping"; Flags: ignoreversion recursesubdirs createallsubdirs

[UninstallDelete]
Type: files; Name: "{app}\.yapfast-enhanced\payload.zip"
Type: files; Name: "{app}\.yapfast-enhanced\payload.next.zip"

[Icons]
Name: "{autoprograms}\Yapfast Enhanced Repair"; Filename: "{app}\.yapfast-enhanced\manager.exe"; Parameters: "repair ""{app}"" --notify"; WorkingDir: "{app}"; IconFilename: "{app}\.yapfast-enhanced\yapfast-enhanced.ico"; Tasks: repairshortcut

[Run]
Filename: "{app}\yapfast.exe"; Description: "Launch Yapfast"; Flags: nowait postinstall skipifsilent

[Messages]
SetupWindowTitle=Setup - Yapfast Enhanced

[Code]
var ShutdownNote: TNewStaticText;

function LoadBrandIcon(Instance: LongWord; Name: String; Kind: LongWord;
  Width, Height: Integer; Flags: LongWord): LongWord;
  external 'LoadImageW@user32.dll stdcall';
function SetBrandIcon(Window: HWND; Message, Which, Icon: LongWord): LongWord;
  external 'SendMessageW@user32.dll stdcall';

function GetCompressedFileSize(Path: String; var High: LongWord): LongWord;
  external 'GetCompressedFileSizeW@kernel32.dll stdcall';

function AllocatedFileSize(Path: String; Fallback: Int64): Int64;
var High, Low: LongWord;
begin
  High := 0;
  Low := GetCompressedFileSize(Path, High);
  if (Low = $FFFFFFFF) and (DLLGetLastError() <> 0) then
    Result := Fallback
  else Result := Int64(High) * 4294967296 + Low;
end;

function InstalledDiskBytes(Path: String): Int64;
var Entry: TFindRec; Child: String;
begin
  Result := 0;
  if FindFirst(Path + '\*', Entry) then begin
    try
      repeat
        { Never follow directory junctions or file symlinks outside this install. }
        if (Entry.Name <> '.') and (Entry.Name <> '..') and
           ((Entry.Attributes and $400) = 0) then begin
          Child := Path + '\' + Entry.Name;
          if (Entry.Attributes and FILE_ATTRIBUTE_DIRECTORY) <> 0 then
            Result := Result + InstalledDiskBytes(Child)
          else Result := Result + AllocatedFileSize(Child,
            Int64(Entry.SizeHigh) * 4294967296 + Entry.SizeLow);
        end;
      until not FindNext(Entry);
    finally
      FindClose(Entry);
    end;
  end;
end;

procedure UpdateStorageEstimate();
var Bytes: Int64;
begin
  { Includes the shared app folder, cold repair payload and restoration backups.
    The 1 GiB installation headroom is not permanent installed storage. }
  Bytes := InstalledDiskBytes(ExpandConstant('{app}'));
  if not RegWriteDWordValue(HKCU,
    'Software\Microsoft\Windows\CurrentVersion\Uninstall\{437563B0-0C9A-4B3B-978E-8D057910AB77}_is1',
    'EstimatedSize', (Bytes + 1023) div 1024) then
    Log('Could not refresh Windows installed-size estimate.');
end;

procedure ApplyBrandIcon(Window: HWND; Path: String);
var SmallIcon, LargeIcon: LongWord;
begin
  { Load both actual Windows sizes; a 256px-only icon left a generic small icon. }
  SmallIcon := LoadBrandIcon(0, Path, 1, ScaleX(16), ScaleY(16), $10);
  LargeIcon := LoadBrandIcon(0, Path, 1, ScaleX(32), ScaleY(32), $10);
  if SmallIcon <> 0 then SetBrandIcon(Window, $80, 0, SmallIcon);
  if LargeIcon <> 0 then SetBrandIcon(Window, $80, 1, LargeIcon);
end;

function ReportText(Path: String): String;
var Text: AnsiString;
begin
  if LoadStringFromFile(Path, Text) then Result := UTF8Decode(Text)
  else Result := 'The operation did not return a report. No unsupported Yapfast version will be modified.';
end;

procedure InitializeWizard();
begin
  ExtractTemporaryFile('yapfast-enhanced.ico');
  ApplyBrandIcon(WizardForm.Handle, ExpandConstant('{tmp}\yapfast-enhanced.ico'));
  WizardForm.WelcomeLabel1.Caption := 'Yapfast Enhanced v{#EnhancedVersion}';
  WizardForm.WelcomeLabel1.Font.Size := 22;
  WizardForm.WelcomeLabel1.AutoSize := True;
  { Keep native DPI-scaled heading metrics; changing only the font clipped text. }
  WizardForm.Caption := 'Setup - Yapfast Enhanced';
  WizardForm.WelcomeLabel2.Caption :=
    'Supports Yapfast {#AppVersion} (Windows x64).' + #13#10#13#10 +
    'Plugins for the Yapfast you already use.' + #13#10#13#10 +
    'Keep your accounts, settings and everyday shortcut. Add a plugin manager and WebAssembly plugin support in the same app.' + #13#10#13#10 +
    'A verified backup of the original application is kept for restoration.' + #13#10#13#10 +
    'Future official updates require a matching Enhanced package. If none is available, the newer official app is kept and plugins pause until a compatible package is published.';
  WizardForm.SelectDirLabel.AutoSize := True;
  WizardForm.SelectDirLabel.Caption := 'Select the existing folder containing yapfast.exe. Setup will close Yapfast, including its tray process. Finish active calls and uploads before continuing. Quitting Yapfast manually from its tray menu may speed up installation.';
  WizardForm.FinishedLabel.Caption := 'You are ready to use plugins.' + #13#10#13#10 +
    'Start Yapfast with your usual shortcut and open Settings > Plugins. Check Enhanced releases from Settings > Enhanced.' + #13#10#13#10 +
    'Remove Yapfast Enhanced from Windows Installed apps to restore the original application. Your account and plugin data stay in place.';
end;

function UpdateReadyMemo(Space, NewLine, MemoUserInfoInfo, MemoDirInfo, MemoTypeInfo, MemoComponentsInfo, MemoGroupInfo, MemoTasksInfo: String): String;
begin
  Result := 'Before installing:' + NewLine +
    'Finish calls and uploads and save your work. Setup closes Yapfast automatically and may force it to close if it does not exit normally.' + NewLine + NewLine +
    'Quitting Yapfast manually, including its tray icon, may speed up installation.';
  if MemoUserInfoInfo <> '' then Result := Result + NewLine + NewLine + MemoUserInfoInfo;
  if MemoDirInfo <> '' then Result := Result + NewLine + NewLine + MemoDirInfo;
  if MemoTypeInfo <> '' then Result := Result + NewLine + NewLine + MemoTypeInfo;
  if MemoComponentsInfo <> '' then Result := Result + NewLine + NewLine + MemoComponentsInfo;
  if MemoGroupInfo <> '' then Result := Result + NewLine + NewLine + MemoGroupInfo;
  if MemoTasksInfo <> '' then Result := Result + NewLine + NewLine + MemoTasksInfo;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var Code: Integer; Report: String;
begin
  Result := '';
  if not FileExists(ExpandConstant('{app}\yapfast.exe')) then begin
    Result := 'This is a plugin-support installer, not a new Yapfast installation. Choose the folder containing your existing yapfast.exe.';
    exit;
  end;
  ExtractTemporaryFile('manager.exe');
  ExtractTemporaryFile('manifest.json');
  Report := ExpandConstant('{tmp}\enhanced-check.txt');
  if ShutdownNote = nil then begin
    ShutdownNote := TNewStaticText.Create(WizardForm);
    ShutdownNote.Parent := WizardForm.PreparingPage;
    ShutdownNote.SetBounds(ScaleX(8), ScaleY(12), WizardForm.PreparingPage.ClientWidth - ScaleX(16), ScaleY(180));
    ShutdownNote.Font.Color := WizardForm.PreparingLabel.Font.Color;
    ShutdownNote.AutoSize := False;
    ShutdownNote.WordWrap := True;
  end;
  ShutdownNote.Caption := 'Closing Yapfast...' + #13#10#13#10 +
    'Quitting Yapfast manually, including its tray icon, may speed up installation.' + #13#10#13#10 +
    'Finish calls and uploads and save your work before continuing. If Yapfast does not exit normally, Setup may force it to close after a short wait.';
  ShutdownNote.Visible := True;
  ShutdownNote.Repaint;
  WizardForm.Repaint;
  if not Exec(ExpandConstant('{tmp}\manager.exe'), 'stop "' + ExpandConstant('{app}') + '" "' + ExpandConstant('{tmp}') + '" --report "' + Report + '"', '', SW_HIDE, ewWaitUntilTerminated, Code) then begin
    ShutdownNote.Visible := False;
    Result := 'Could not start automatic shutdown. Quit Yapfast from its tray menu and retry.';
    exit;
  end;
  ShutdownNote.Visible := False;
  if Code <> 0 then begin
    Result := ReportText(Report);
    exit;
  end;
  if not Exec(ExpandConstant('{tmp}\manager.exe'), 'check "' + ExpandConstant('{app}') + '" "' + ExpandConstant('{tmp}') + '" --report "' + Report + '"', '', SW_HIDE, ewWaitUntilTerminated, Code) then
    Result := 'Could not start the compatibility check.'
  else if Code <> 0 then Result := ReportText(Report);
end;

procedure CurStepChanged(CurStep: TSetupStep);
var Code: Integer; Report: String;
begin
  if CurStep = ssPostInstall then begin
    Report := ExpandConstant('{tmp}\enhanced-install.txt');
    WizardForm.StatusLabel.Caption := 'Installing plugin support and verifying the original backup...';
    if not Exec(ExpandConstant('{app}\.yapfast-enhanced\manager.exe'), 'install "' + ExpandConstant('{app}') + '" "' + ExpandConstant('{app}\.yapfast-enhanced\payload') + '" --report "' + Report + '"', '', SW_HIDE, ewWaitUntilTerminated, Code) then
      RaiseException('Could not start the in-place installer.')
    else if Code <> 0 then RaiseException(ReportText(Report));

  end;
  if CurStep = ssDone then UpdateStorageEstimate();
end;

function InitializeUninstall(): Boolean;
var Code: Integer; Report: String;
begin
  Report := ExpandConstant('{tmp}\enhanced-restore.txt');
  Result := Exec(ExpandConstant('{app}\.yapfast-enhanced\manager.exe'), 'restore "' + ExpandConstant('{app}') + '" --report "' + Report + '"', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Result := Result and (Code = 0);
  if not Result then MsgBox(ReportText(Report) + #13#10 + 'Uninstall was stopped so your backups are kept. Completely quit Yapfast and retry.', mbError, MB_OK);
end;


procedure InitializeUninstallProgressForm();
begin
  ApplyBrandIcon(UninstallProgressForm.Handle,
    ExpandConstant('{app}\.yapfast-enhanced\yapfast-enhanced.ico'));
end;
