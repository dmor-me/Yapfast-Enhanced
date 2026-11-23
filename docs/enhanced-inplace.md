# Enhanced installation and recovery for Yapfast 0.3.0

Enhanced 0.3.0.0 targets the official Windows x64 Yapfast 0.3.0 executable.
It replaces that executable with a plugin-enabled build of the same upstream
version and adds its WebAssembly helper. It is not a universal injector.

See [validation status](compatibility-0.3.0.md) before installing this experimental
branch. The 0.4.1.0 checkout and its installation are independent.

## Compatibility and installation

The installer checks this exact original executable SHA-256:

`ae1c4a99cd2ed9db85ac234dcc80ef4efdf2ee812cb980d071fbf6ffcf82e8f0`

A different version or fingerprint is refused. Do not point this installer at a
0.4.x installation. Use a disposable copy of the official 0.3.0 portable app for QA.

Save unfinished work first. Setup asks the selected installation to quit,
including its tray process. After a five-second grace period it can stop only
processes whose executable paths match that installation. Closing Yapfast
manually may speed this up. Windows Restart Manager is disabled.

## Recovery

The manager keeps verified compressed archives for the original executable and
the Enhanced repair payload. Account data and installed-plugin data are not
part of these archives and are not removed by install, repair or restore.

The installed manager lives under `.yapfast-enhanced/manager.exe`. Its commands
accept the app directory, with quotes around paths containing spaces:

```powershell
& '.\.yapfast-enhanced\manager.exe' repair 'C:\path\to\Yapfast'
& '.\.yapfast-enhanced\manager.exe' restore 'C:\path\to\Yapfast'
```

Repair uses the verified local payload first; a missing payload can require the
matching GitHub release. Restore verifies and restores the original binary.
Unneeded downloads/backups are cleaned conservatively. Plugin HTTP temporary
files are owned by their response and removed when it is released.

## Updates

Upstream 0.3.0 has no About updater. Enhanced's own checker is in **Settings →
Enhanced**, with manual and optional six-hour checks. Automatic installation is
unavailable. Releases must contain an asset for the exact supported base hash;
a later Yapfast release is not automatically compatible with this build.
