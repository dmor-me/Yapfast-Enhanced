#ifndef EnhancedVersion
  #error EnhancedVersion is required
#endif
#ifndef ManagerExe
  #error ManagerExe is required
#endif
[Setup]
AppId=YapfastEnhancedUniversalBootstrap
AppName=Yapfast Enhanced Universal Installer
AppVersion={#EnhancedVersion}
VersionInfoVersion={#EnhancedVersion}
AppVerName=Yapfast Enhanced Universal Installer
AppPublisher=Yapfast Enhanced
DefaultDirName={localappdata}\Programs\Yapfast
DisableDirPage=no
DisableProgramGroupPage=yes
CreateAppDir=no
DirExistsWarning=no
Uninstallable=no
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputBaseFilename=Yapfast-Enhanced-Universal-Installer
SetupIconFile=branding/yapfast-enhanced.ico
WizardStyle=modern dark hidebevels includetitlebar
WizardBackColor=#141518
WizardSizePercent=120,110
WizardImageFile=branding/sidebar.bmp
WizardSmallImageFile=branding/header.bmp
WizardImageStretch=yes
WizardImageBackColor=#141518
WizardSmallImageBackColor=#141518
DefaultDialogFontName=Segoe UI
Compression=lzma2
SolidCompression=yes
CloseApplications=no
RestartApplications=no
DisableWelcomePage=no
[Files]
Source: "{#ManagerExe}"; DestName: "manager.exe"; Flags: dontcopy
Source: "branding/yapfast-enhanced.ico"; DestName: "enhanced.ico"; Flags: dontcopy
[Messages]
SetupWindowTitle=Setup - Yapfast Enhanced
[Code]
var
  Notice: TNewStaticText;
  InstallDirectoryPage: TInputDirWizardPage;

function SelectedYapfastDirectory(): String;
begin
  Result := Trim(InstallDirectoryPage.Values[0]);
end;

function ValidYapfastDirectory(): Boolean;
begin
  Result := (SelectedYapfastDirectory() <> '') and
    FileExists(AddBackslash(SelectedYapfastDirectory()) + 'yapfast.exe');
end;

function LoadBrandIcon(Instance: LongWord; Name: String; Kind: LongWord;
  Width, Height: Integer; Flags: LongWord): LongWord;
  external 'LoadImageW@user32.dll stdcall';
function SetBrandIcon(Window: HWND; Message, Which, Icon: LongWord): LongWord;
  external 'SendMessageW@user32.dll stdcall';
procedure InitializeWizard();
var IconPath: String;
begin
  ExtractTemporaryFile('enhanced.ico');
  IconPath := ExpandConstant('{tmp}\enhanced.ico');
  SetBrandIcon(WizardForm.Handle, $0080, 0, LoadBrandIcon(0, IconPath, 1, 16, 16, $0010));
  SetBrandIcon(WizardForm.Handle, $0080, 1, LoadBrandIcon(0, IconPath, 1, 32, 32, $0010));
  WizardForm.Caption := 'Setup - Yapfast Enhanced';
  WizardForm.WelcomeLabel1.Caption := 'Yapfast Enhanced';
  WizardForm.WelcomeLabel2.Caption := 'Universal online installer' + #13#10#13#10 +
    'Detects your exact Yapfast build and downloads its matching Enhanced installer from GitHub.' + #13#10#13#10 +
    'Internet access is required. The version-specific installer is verified before it opens. It handles installation, backup and restoration.' + #13#10#13#10 +
    'Unsupported versions are left unchanged. Yapfast is never upgraded or downgraded automatically.';
  WizardForm.FinishedLabel.Caption := 'The matching Enhanced installer completed. Start Yapfast normally and open Settings > Plugins.';
  { CreateAppDir=no disables Inno's built-in directory page and maps the app constant
    to Windows. A dispatcher must keep its own selected directory instead. }
  InstallDirectoryPage := CreateInputDirPage(wpWelcome,
    'Select Yapfast Installation', 'Where is your existing Yapfast installed?',
    'Select the folder containing yapfast.exe. Save your work and finish calls/uploads before continuing.',
    False, '');
  InstallDirectoryPage.Add('Yapfast installation folder:');
  InstallDirectoryPage.Values[0] := ExpandConstant('{localappdata}\Programs\Yapfast');
  { Folder picker initialized. }

end;
function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := True;
  if (CurPageID = InstallDirectoryPage.ID) and not ValidYapfastDirectory() then begin
    MsgBox('No yapfast.exe was found in:' + #13#10 + SelectedYapfastDirectory() + #13#10#13#10 +
      'Choose the existing Yapfast folder, not its parent folder or an installer download folder.', mbError, MB_OK);
    Result := False;
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var Code: Integer; Report, AppDirectory: String; MessageText: AnsiString;
begin
  Result := '';
  AppDirectory := SelectedYapfastDirectory();
  if not ValidYapfastDirectory() then begin
    Result := 'Choose an existing Yapfast installation containing yapfast.exe. This installer does not install plain Yapfast.';
    exit;
  end;
  ExtractTemporaryFile('manager.exe');
  Report := ExpandConstant('{tmp}\universal-report.txt');
  if Notice = nil then begin
  Notice := TNewStaticText.Create(WizardForm);
  Notice.Parent := WizardForm.PreparingPage;
  Notice.SetBounds(ScaleX(8), ScaleY(12), WizardForm.PreparingPage.ClientWidth - ScaleX(16), ScaleY(180));
  Notice.Font.Color := WizardForm.PreparingLabel.Font.Color;
  Notice.AutoSize := False;
  Notice.WordWrap := True;
  end;
  Notice.Visible := True;
  Notice.Caption := 'Finding and downloading the matching Enhanced installer...' + #13#10#13#10 +
    'This needs internet and may take a few minutes. A separate installer will open for the detected version. Review it to continue.';
  Notice.Repaint;
  WizardForm.Repaint;
  if not Exec(ExpandConstant('{tmp}\manager.exe'), 'bootstrap "' + AppDirectory + '" --report "' + Report + '"', '', SW_HIDE, ewWaitUntilTerminated, Code) then
    Result := 'Could not start version detection. Try the version-specific offline installer.'
  else if Code <> 0 then begin
    if LoadStringFromFile(Report, MessageText) then Result := UTF8Decode(MessageText)
    else Result := 'Detection, download or installation did not complete. No other Yapfast version was selected.';
  end;
  Notice.Visible := False;
end;
