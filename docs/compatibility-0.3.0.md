# Enhanced 0.3.0 compatibility experiment

Status: local Windows test installer built and validated against official 0.3.0.
No release has been published. Test with disposable app/data folders first.

## Isolation and source provenance

- Branch: `compat/yapfast-0.3.0`.
- Baseline: official `v0.3.0`, commit `03862f6394fad09b541d731181b819a3c9713d3f`.
- Shared Enhanced components: saved main commit `0ffb0f6e` (initial reusable modules came from `4067d7da`).
- Intended product version: `0.3.0.0`; this is not a published release.
- The original `main` checkout and its 0.4.1.0 installer remain independent.
- No GitHub push or real-app installation is part of this experiment.

The branch begins with genuine Yapfast 0.3.0 source. It does not rename the
0.4.1 app to make it appear compatible. The first full three-way backport exposed
conflicts in settings, chat, GIF handling, notifications and update code. That
attempt was cleared before bringing over the reusable modules deliberately.
There are no unresolved merge conflicts.

## Current availability

| Component | State |
| --- | --- |
| Plugin manifest/API, registry, permission gate, network/store code | Ported from Enhanced; tested separately |
| Extism/WebAssembly helper | Optimized helper built; eight real WebAssembly integration cases pass |
| Repair/restore manager and numeric release decisions | Ported and tested independently with 0.3.0-specific version/fingerprint |
| Plugin backend bridge into 0.3.0 chat/actions | Validated: backend unit tests plus real WebAssembly execution |
| Plugins settings page and native plugin media browser | Native UI tests pass; real local-server thumbnail loading passes |
| Enhanced settings and optimization tools | Native UI/persistence tests and optimized app smoke check pass |
| Version-specific Windows installer | Built locally; isolated installation, offline repair and exact restoration pass |

The app now enables the existing plugin bridge by default. Core tests exercise
real plugin installation and permission decisions, and the optimized helper has
passed real WASM execution tests. Native UI, app smoke and installer recovery
checks pass; these do not replace live-account/manual plugin testing.
The SDK/API versions remain independent of the Enhanced product version.

## Older-model adaptations and limits

- Message edits still use the 0.3.0 composer. The `message.pre_edit` hook uses that
  composer; a cancelled held edit cannot turn into a new message or overwrite the
  restored ordinary draft.
- Member lists stay in the original Vec model. The plugin world receives one
  shared snapshot per update, reused across actions. Discord bot flags come from
  the existing names cache.
- Native favorite previews accept only a source already in the account's saved
  favorites and reuse the original thumbnail/cache path.
- Profile overrides support display name, avatar, banner and pronouns. The 0.3.0
  profile renderer has no name-colour style or avatar-decoration surface; those
  profile-card effects are not rendered. Member-list name colours are supported.
- 0.3.0 does not retain a separate account username, so that optional API value is
  absent. Account identity and display name remain available.
- Upstream 0.3.0 has no About updater. Enhanced supplies its own manual/six-hour
  GitHub release checker. There is no official-update handoff/consent page to
  intercept on this base. Automatic update installation remains unavailable.
- The newer `outline`/`outline-field` palette roles map to 0.3.0's rendered
  `edge` colour; `outline-chat` maps to `line`. They cannot distinguish newer
  border roles absent from the old renderer.
- Newer upstream features such as 0.4 keybind settings, inline message editing,
  bot interaction UI and profile cosmetics are not part of this backport.

## Upstream Windows test fixture

The portable ZIP came from the official
[0.3.0 release](https://github.com/yapfast/yapfast/releases/tag/v0.3.0).
Only the executable was extracted to the ignored `.local/upstream-0.3.0/` test
folder, after checking the release asset digest.

- Portable ZIP SHA-256: `f720b6910e05a632fad64c8419f7af6733028a7453ec693fa3ecc64c68044a6e`.
- Original executable SHA-256: `ae1c4a99cd2ed9db85ac234dcc80ef4efdf2ee812cb980d071fbf6ffcf82e8f0`.

No live executable, account data, registry installation entry, or tray session
was replaced. Tests must use disposable app/data folders.

## Build approach

Keep the original app/Slint versions (`0.3.0` / `1.18.1`) while adapting integration
points. The shared components reuse the tested Enhanced dependency lock where
compatible; Cargo resolves the older app dependencies separately. Avoid compiling
multiple Slint UI builds at once.

On Windows, use the established native toolchain setup. The Linux
`scripts/warm-worktree` helper requires Nix and reflinks, so it is not suitable for
this Windows NTFS worktree; focused component builds can reuse the existing
native dependency cache without modifying the main source or published artifacts.

## Validation and remaining gates

Completed on Windows using disposable processes/profiles:

- Backend unit and plugin integration suites: 519 passed before the added preview
  regressions; actual WebAssembly execution includes startup, filtering, timeout,
  idle wake, decoration and shutdown cases.
- Native app suite: 213 passed, 6 skipped (debug-only/platform or system-changing
  checks). Additional profile/memory and palette checks use actual UI bindings.
- Native no-plugin test executable builds successfully.
- Discord service: 50 cases pass in separate processes, avoiding Windows port
  reuse; the old Unix socket temp-path assumption is adapted on Windows.
- Discord backend: 9 local-server cases pass, including reopening channels.
- Native favorite previews: saved source and page URLs load actual thumbnails;
  unknown URLs and signed-out requests are rejected. The cache is temporary.
- Example media environment: passes with local FFmpeg/ffprobe test tools.
- The shared scheduler's pending-reply idle regression reproduces before the
  fix and passes afterward; all 29 scheduler integration cases pass. Completing
  work now wakes the watchdog to arm idle cleanup. This change is scoped to the
  compatibility branch; main remains unchanged.
- Installer manager: 38 unit tests pass. Scoped shutdown fixtures take about
  0.47 seconds when responsive, 5.2 seconds when unresponsive, and leave a second
  installation untouched.
- Shared components: API 65, helper unit 80, additional host integration 13 and
  11, plugin-system 216, and scheduler 29 cases pass.
- Source publication guards, nine packaging tests, documentation links and
  committed WebAssembly fixture hashes pass.

Optimized app/helper builds and native app smoke checks pass. All eight runtime
integration tests pass again against the final optimized helper. The local Inno
installer compiles and its bundled manager passes installation, offline repair
after removing the helper, and byte-exact restoration of the official executable.
An incompatible fingerprint is refused by check/stop/install without changing
its files. An unrelated second installation and a fake account marker remain
unchanged. The real installer UI was not run against the installed app or system
uninstall registry. Live-account plugin/FGF checks remain manual follow-ups.
No main checkout, normal account profile, installed app or GitHub remote is changed.

The platform's Nix shell is unavailable on Windows; checks use the established
native toolchain. Linux-only/debug-tree/system-registry tests are not claimed to
run here. On Windows, run fake service cases in separate processes (as nextest
does), with Bun and an FFmpeg build containing libx264 available for media fixtures.

Private validation logs and measured outputs live in ignored `.local/`.

## Local test build

- Product: Enhanced `0.3.0.0` for official Windows x64 Yapfast `0.3.0`.
- Compiled source checkpoint: `66ae4f97e88c21435b50c9a96778ae115db3c60a`.
- Base metadata: `Yapfast v0.3.0 · Stable · 66ae4f97 · 2026-11-22`.
- Local artifact: `.release/v0.3.0.0/Yapfast-Enhanced-Installer.exe`.
- Installer size: 53,850,066 bytes (about 51.4 MiB).
- Installer SHA-256: `b08fd84712fc65be331921dbc816ba0219d85a84b28d3a0e7202fe1f62787ea7`.
- No code signing or public release is claimed. Keep the installer together with
  its source checkpoint and `SHA256SUMS.txt` when distributing test builds.

The version-specific installer includes its payload and can install offline.
Marketplace discovery and remote update checks require network access. Plugin
permissions and integrity hashes remain enforced; these do not establish that
third-party plugin behavior is trustworthy.

## Zero-based initial release

Enhanced `0.3.0.0` uses release title **Enhanced 0.3.0.0** and tag `v0.3.0.0`.
Installer: `.release/v0.3.0.0/Yapfast-Enhanced-Installer.exe`
(53,850,066 bytes), SHA-256 `b08fd84712fc65be331921dbc816ba0219d85a84b28d3a0e7202fe1f62787ea7`.
Compiled source: `66ae4f97e88c21435b50c9a96778ae115db3c60a`. The app reports `Enhanced 0.3.0.0`.

Release parsing and manager tests, native smoke, installation over the previous
Enhanced build, offline repair, exact original restoration and unsupported-binary
refusal pass. Normal account data and the installed app were not used for QA.
