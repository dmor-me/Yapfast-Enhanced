# Changelog

## 0.4.0.0 - 2026-11-23

- Initial native compatibility build for official Yapfast 0.4.0.
- Includes Enhanced plugin management, WebAssembly runtime, optional memory tools
  and version-matched installation/recovery.
- Preserves 0.4.0 push-to-talk controls; does not add the 0.4.1 keybind page.

Enhanced versions use `<Yapfast major>.<minor>.<patch>.<Enhanced revision>`.
The final number starts at 0 and increases for Enhanced updates on that base.

## 0.4.1.0 - 2026-11-18

Initial Enhanced release source, targeting official Yapfast 0.4.1 on Windows 11 x64.

- Zero-based initial revision and canonical `v0.4.1.0` release tag.
- Optional universal online installer with exact-build detection and verified handoff.

- Native Rust/Slint plugin manager and Extism/WebAssembly runtime helper.
- Reviewed, commit-pinned GitHub marketplace and GitHub/Codeberg URL installs.
- Local imports, permission review, settings, logs, manual updates and rollback.
- Native plugin media browser with viewport-aware preview retention.
- Enhanced settings with version copying, GitHub link and optional six-hour checks.
- Optional DM and server-history memory release, effective immediately.
- Native plugin preview memory budget and manual unused-preview release.
- Size reporting and confirmed cleanup of cached media older than seven days.
- Process RAM/CPU information and restart with plugins paused or enabled.
- Version-specific Windows installer, automatic app shutdown and verified restoration.
- Explicit pre-update Enhanced availability/uncertainty confirmation with cancellation.
- Bounded, installation-scoped shutdown with visible manual-close guidance.
- On-demand per-plugin disk usage with saved-data and rollback breakdowns; shared cache is reported once on Installed.
- Compressed, SHA-256-verified offline recovery archives with legacy migration.
- Conservative recovery/update cleanup that protects pending state and active downloads.
- Response-owned plugin HTTP temporary files, cleaned on completion/error/cancellation.
- Measured installed-size reporting without the temporary free-space allowance.
- Ribbon E branding for the application, settings and installer.

Automatic Enhanced update installation remains unavailable. Actual per-plugin
allocation telemetry remains unavailable. Each new official Yapfast executable
requires a matching Enhanced compatibility build. See the
[installation and restoration guide](docs/enhanced-inplace.md).
