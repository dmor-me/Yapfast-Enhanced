# Enhanced 0.1.0 compatibility experiment

Status: local Windows test installer built and validated; not publicly released.

- Base: official Yapfast v0.1.0, 74848a170d545c153f99f4a1211277d450f05f82.
- Branch: compat/yapfast-0.1.0, isolated from existing compatibility/main worktrees.
- Target product: Enhanced 0.1.0.0; SDK/API versions remain independent.
- Reuses the tested API 1, Extism/WebAssembly runtime and manager from 0.2.0.
- Preserves the original fullscreen settings shell, adding Plugins and Enhanced.

Native UI/backend, real WebAssembly, official input integrity, optimized build,
and isolated installer repair/restore checks pass. Older API limitations are
listed below. No normal profile, installed app or GitHub remote is changed.

## Current validation and explicit limits

- Slint 1.18.1 UI syntax/type validation passes.
- Backend unit/integration suites: 344 passed, 19 ignored in the routine run.
  The ignored local-server scenarios were exercised separately below.
- Nine source-release tests, documentation links and WASM fixture hashes pass.
- Official Windows portable archive and executable digests are verified; the
  original executable also passes its native stack smoke check.
- Newer game-activity control, invite pausing, and server emoji/sticker uploads
  are absent from this base. Those calls return informative unsupported errors
  with no side effects; status/custom-status and other implemented calls remain.
- The original settings shell is retained. Controlled plugin switches, choices,
  input-focus notifications and slider formatting are small added primitives;
  existing controls keep their original defaults.
- Cached pictures use the older read-only filesystem cache path. No network
  fetch is substituted for a cache miss.

No source/installer from a newer compatibility branch or installed app is changed.

## Native integration checkpoint

- Native app suite: 145 passed, 4 ignored.
- Backend unit/integration suites: 344 passed, 19 ignored in the routine run,
  including eight real WebAssembly cases. The local-server cases are separate.
- Installer manager: 36 library and 2 command-line tests pass.
- Separate local Discord suites: 159 unit tests, one service scenario, one
  example environment and eight backend history/preview scenarios pass.

## Final Windows validation

The optimized app and helper build successfully. All eight real WebAssembly
cases pass against the final helper, and the shipped app passes its native stack
smoke check. The no-plugin build and local Discord service/history/favorite
preview checks pass. Installation, offline repair and exact original restoration
pass in disposable app folders. Modified binaries and genuine official 0.3.0
are refused without changes. Scoped shutdown fixtures pass and leave a second
installation running. The installer itself was not run against the live system
uninstall registry. No normal account or real-account FGF was used for QA.

This unsigned test build preserves the older renderer's limits. API 1 is reused,
but newer game-activity and server-management calls listed above report explicit
unsupported errors. No newer upstream keybinds, profile cosmetics or desktop
integration are implied. Linux and Matrix builds were not validated on Windows.

Compiled checkpoint: `b500b55df5eb153abf0c0c73f56d6f3a1eed0d55`.

Base metadata: `Yapfast v0.1.0 · Stable · b500b55d · 2026-11-22`.

Local installer: `.release/v0.1.0.0/Yapfast-Enhanced-Installer.exe`
(48,457,683 bytes). SHA-256: `0856d17f1821af3fd3e20fe2a690304eea965595e18c43f823259e4b6cd0c748`.

## Renderer and publication limits

The older profile renderer supports display name, avatar, banner and pronouns;
newer profile-card name styles and avatar decorations are unavailable. Member
name colours remain supported. The account's separate username is unavailable.
Newer outline roles map to the existing edge/line palette. API 1 compatibility
does not guarantee every plugin feature is supported on this older host.

Payload hashes and permission checks remain enforced. They do not prove
third-party plugin behavior trustworthy. This unsigned installer and its source
are local test artifacts; no GitHub release, push or tag was created.

Private test logs and disposable fixtures are excluded from the source tree.

## Plugin settings clipping correction

The Plugins page now receives the full settings viewport, so its nested lists
and dialogs cannot collapse to their minimum content size. Advanced options
scroll when needed while the Done action stays visible. Other settings retain
the original form scrolling.

The native geometry regression passes at 800x560, 1024x600, 1280x720 and
1920x1080 in both Installed list and card modes, including the Advanced dialog
and its final action. The routine desktop suite passes 145 tests (five ignored,
including the geometry case executed separately with Slint debug information).
The optimized build, native smoke and disposable install/offline repair/exact
restore/refusal checks pass again. No live account, installed app or registry
was used. Real WebAssembly tests passed before this UI-only change; the shipped
helper is unchanged.

Upgrading the prior Enhanced 0.1.0.0 build to the corrected installer also
passes in a disposable folder, preserving an unrelated settings marker.

## Zero-based initial release

Enhanced `0.1.0.0` uses release title **Enhanced 0.1.0.0** and tag `v0.1.0.0`.
Installer: `.release/v0.1.0.0/Yapfast-Enhanced-Installer.exe`
(48,457,683 bytes), SHA-256 `0856d17f1821af3fd3e20fe2a690304eea965595e18c43f823259e4b6cd0c748`.
Compiled source: `b500b55df5eb153abf0c0c73f56d6f3a1eed0d55`. The app reports `Enhanced 0.1.0.0`.

Release parsing and manager tests, native smoke, installation over the previous
Enhanced build, offline repair, exact original restoration and unsupported-binary
refusal pass. Normal account data and the installed app were not used for QA.
