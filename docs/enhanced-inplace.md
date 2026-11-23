# Windows installer: plugin support in your existing Yapfast

Run the latest `Yapfast-Enhanced-Installer.exe` after finishing active calls and uploads. Setup closes Yapfast, including its tray process. Select the folder containing your existing `yapfast.exe`; the default is `%LOCALAPPDATA%\Programs\Yapfast`. This build supports the exact official Windows x64 Yapfast 0.4.0 executable with SHA-256 `32d75df588581a0120157ab15d379599e3c9a98ac978e62f63a2b5fb75c3e69c`. Other builds are refused before replacement.

The installer backs up the official executable, replaces it with the compatible plugin-enabled build, and adds the WebAssembly runtime helper to the same installation. Launch your usual Yapfast shortcut. Existing account data and shortcuts are used. Open Settings > Plugins to browse, enable, disable, update or uninstall plugins, or import a local plugin. Yapfast plugins use its own manifest and API; Discord JavaScript plugins are incompatible.

This is version-specific native executable replacement. The Rust/Slint plugin hooks are compiled into the application; this does not inject a generic JavaScript patcher into arbitrary official builds. The source integrates the existing Extism runtime from `blockgameentity/yapfast`'s `feat/plugins` branch rather than implementing a second runtime. See [plugin details and permissions](plugins/README.md#enhanced-integration).

## Official updates

Before downloading/installing an official update, Enhanced checks its release
channel for a package listed for the destination Yapfast version. About shows
an explicit confirmation with Continue/Cancel. Missing support warns that the
updated app may run without Enhanced; a failed check is shown as unconfirmed,
not as compatible. A listed package is not proof of the exact executable:
that fingerprint is still verified after the official update. Cancelling the
prompt cannot be reversed by a late network response. No official installer
is downloaded or run until confirmation.

Normal in-app updates still use Yapfast's existing official download verification. When plugin support is installed, the verified updater hands off to the manager. It waits for Yapfast to exit, installs the official update into the same folder, looks for a matching Enhanced package, and restarts the app. A matching package restores plugin support. Without one, the newer official app stays installed and an informative message explains that plugins are paused. It never downgrades an unknown updated executable to the old supported version.

The download channel is `dmor-me/Yapfast-Enhanced` on GitHub. Each new official version needs a corresponding rebuilt and tested Enhanced package. Compatibility depends on maintaining that release channel. Enhanced release packaging and publishing are currently manual. See the [release guide](releasing-enhanced.md).

Manual/external updaters can bypass the handoff. Run the optional Yapfast Enhanced Repair shortcut, rerun a compatible installer, or run `.yapfast-enhanced\manager.exe repair "<Yapfast folder>" --notify` once the app is closed. Repair first uses a compatible local payload, otherwise it checks the GitHub channel. There is no resident background service.

## Restore

Use Windows Installed apps > Yapfast Enhanced > Uninstall. It verifies and restores the original executable, restores any pre-existing helper, and preserves user/plugin data. The official Yapfast uninstaller is separate. An unknown newer official executable is preserved. Damaged backups or a running app stop restoration with an error instead of deleting recovery files. Backups/state can remain under `.yapfast-enhanced` for recovery.

## Integrity and limits

The manager compares the exact official, patched and helper SHA-256 fingerprints, checks Windows x64 executable headers, rejects unsafe paths, requires the app to be closed, and keeps verified original backups. The application pins the installed manager's hash before handing it an update. Compatibility downloads require HTTPS, a release asset from the configured repository, GitHub's SHA-256 asset digest, bounded archive size and matching inner executable fingerprints. Only the expected archive entries are extracted.

Hashes protect against corruption; they are not independent publisher signatures. These development executables and the installer are unsigned. A compromised release account or writable installation directory is outside this trust boundary. The privileged manager must be trusted. WebAssembly permissions and limits reduce plugin access but are not an audited security guarantee; inspect requested permissions and install trusted plugins. The Enhanced plugin catalog is hosted at `https://github.com/dmor-me/yapfast-plugin-registry`, separately from the Enhanced compatibility release channel.

## Build and publish compatible packages

Build the root app and runtime helper in release mode with `YAPFAST_RELEASE=1` so official update checks remain enabled. Build `tools/yapfast-enhanced-manager` separately with its own Cargo.lock. Then run:

```text
python scripts/package-enhanced-inplace.py --upstream-exe <official-yapfast.exe> --upstream-sha256 <verified-sha256> --app-version <version>
```

The script creates the installer, its checksums, a local payload and `yapfast-enhanced-for-<official-sha256>.zip`. Building the installer requires Inno Setup 6. Publishing code and a non-prerelease compatibility ZIP to the configured public GitHub repository is a separate maintainer action. Publish corresponding source under the application's GPL obligations; signing, release automation and future compatibility builds remain follow-ups.

## Validation before publication

Run the installer-manager unit and CLI tests, plugin fixture checks and native
application smoke test. On an isolated copy of the supported official app, check
installation, tray shutdown, a real plugin invocation and restoration. Preserve
an account-data marker through that loop. Record the tested source revision and
binary hashes with the release, rather than treating old test counts as proof
of a new build. See the [release guide](releasing-enhanced.md).

## Enhanced release checks

Settings > Enhanced shows the Enhanced version and supported base, Copy version, Check now, result messages and View release. It uses the public GitHub releases API for `dmor-me/Yapfast-Enhanced`. The optional Check for updates preference enables a check at startup and every six hours. Automatic installation remains unavailable. The same page contains the Enhanced GitHub link and [Enhanced Tools](enhanced-tools.md).

Publish stable Enhanced releases with their own version tags, using `v0.4.0.0`. The first three numbers identify the supported Yapfast version; the last is the Enhanced revision, starting at 0. Older three-part release tags remain readable as revision zero. Attach `Yapfast-Enhanced-Installer.exe` and the matching `yapfast-enhanced-for-<official-sha256>.zip`. The checker compares stable versions numerically, ignores drafts/prereleases and source-only releases, and checks the compatibility asset before describing it as available for this app build. It links only to this repository's release pages. Empty channels say no releases have been published, rather than claiming the app is current. Errors and oversized responses are reported in the page. The existing repair downloader still verifies assets separately; the check itself downloads no executable.

The Add from URL example uses GitHub. Supported HTTPS Git repositories on GitHub and Codeberg remain accepted.
Maintainers: bump `VERSION` and set `BASE` in `src/update/enhanced_releases.rs` for each compatible build before publishing; `BASE` must match the verified official executable fingerprint used by the packaging script.


## Installer branding and Enhanced settings

The Enhanced UI lives in `ui/enhanced_settings.slint`, with its global data in
`ui/enhanced_updates.slint` and behavior in `src/ui/enhanced_tools.rs`.
The Plugins tabs are Marketplace, Installed and Add from URL.

The installer uses a dark native wizard, Segoe UI, custom header and sidebar art,
and Ribbon E in the original `#FFBF47` and `#1D1607` palette. Its window title is
`Setup - Yapfast Enhanced`. App, tray, notification, installer, uninstaller and
Repair icons use this branding. The existing main shortcut is preserved.
DPI-aware heading metrics and centered sidebar text support scaled displays.
Native dark styling can fall back for high-contrast accessibility.

See [Enhanced branding](branding/README.md) for active assets and regeneration.
The standalone E marks the Enhanced settings entry between Plugins and Advanced.

## Installed storage

The installer keeps offline recovery as compressed ZIP archives: `payload.zip`
contains the current Enhanced executables and manifest, and `backups/<sha>.exe.zip`
contains the exact original executable. A preexisting helper is also archived
when restoration needs it. Live executables stay ordinary, uncompressed files.
Setup retains its existing LZMA compression: the manager builds the on-disk
repair ZIP from the temporary extracted payload, then removes that payload.
This avoids packing a ZIP inside the installer and inflating the download.
Archives are streamed, bounded, and checked against the uncompressed SHA-256
before use. Installation, repair, and restoration can take extra time to extract
and verify archives; normal chat and plugin execution do not read them.

Legacy raw backups remain readable. A successful install migrates required raw
copies only after verifying their archives. It then prunes verified obsolete
backups and the exact old three-file `payload/` layout. Cleanup requires a
complete installed state, matching live files, a valid current offline payload,
and valid original backups. Interrupted operations, unknown files, and modified
copies are retained. No account, installed-plugin, media-cache or upload data is
removed. The original backup is still retained by the manager after restore.

Downloads use temporary session directories with ownership markers and a
cross-process lease held through installation. Normal completion and failure
remove that session; later maintenance can remove inactive owned sessions left
by a crash. Active sessions and unrecognized legacy downloads are never swept.
`manager.exe cleanup <app-folder>` performs the same conservative maintenance;
it does not clear shared compilation or media caches. No cleanup timer runs in
the main application.

After installation, Windows Installed apps reports allocated file bytes across
the selected Yapfast application folder, including its shared live files, repair
payload and backups. It excludes the temporary 1 GiB free-space allowance and
skips filesystem links. This estimate is refreshed by the installer, not live
after later changes. Windows can also retain a separate upstream Yapfast entry;
these two entries overlap and must not be added together.

## Installer shutdown feedback

The ready page asks users to finish calls/uploads and save work before Setup
closes the app. Preparing to Install visibly explains that quitting Yapfast
manually from its tray menu may speed installation. The manager skips shutdown
when the selected app and helper are already closed. It first requests a
normal exit, allows five seconds, then may terminate only processes whose full
executable paths match that selected installation. Force-closing can lose
unsaved work. It allows two more seconds for file locks to clear, then reports
an actionable error rather than waiting indefinitely in Windows Restart Manager.
Other installations and unrelated processes are not closed.
