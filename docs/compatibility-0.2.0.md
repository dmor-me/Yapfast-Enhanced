# Enhanced 0.2.0 compatibility experiment

Status: local Windows test installer built and validated. No public release is
implied. Test with disposable app/data folders first.

- Base: official Yapfast v0.2.0, c4c9f5d085546c72364677cb770f466fad8b9fbd.
- Branch: compat/yapfast-0.2.0, isolated from the 0.3.0 and main checkouts.
- Target product: Enhanced 0.2.0.0.
- Reuses the 0.3.0 backport's API 1, Extism/WebAssembly runtime and manager.
- Keeps 0.2.0's own chat/navigation model; newer upstream features are not implied.

Native UI, backend/runtime, optimized builds, official binary integrity and
isolated installer recovery checks pass. Live-account/manual plugin acceptance
remains a follow-up. No installed app, account data or GitHub
remote is changed by this work.

## Current validation

- Slint 1.18.1 UI syntax/type validation passes.
- Refreshed backend suite: 416 unit tests pass, including MP3/Ogg audio-only
  decoding and list-generation regressions. Integration suites add 48 passing
  cases, including eight real WebAssembly cases (464 total; 21 ignored).
- Native desktop app suite: 183 passed, 5 ignored. Includes the plugin member
  viewport throttling regression.
- Manager tests: 36 library and 2 command-line tests pass.
- Nine source-release tests and committed WebAssembly fixture hashes pass.
- Official portable archive SHA-256:
  `0f25e73117b9914a05e8ca15aed24cda9c90d5037e8ab4f2f388208187c12a16`.
- Official executable SHA-256:
  `bcc7ffeee254181bfd4e9c4e6830def46ae5fb988fe9c30ea10c5921d3c17ff9`.

## Older-base adaptations

- Room lists retain 0.2.0's Vec model. A generation counter updates only when
  lists change, so plugin reads reuse the published snapshot.
- Server mentions and channel lookup use 0.2.0's direct server model.
- Plugin MP3/Ogg sounds reuse the old native media decoder with an audio-only
  mode, the existing resampler, and the plugin's ten-second output limit. This
  adds no soundboard UI or new upstream codec features.
- Pause/resume restarts use the older startup path. Newer desktop deep-link
  registration and upstream updater features are not imported.

## Additional native validation

- Native desktop test executable also compiles with plugin support disabled.
- The 0.2.0 Discord service's monolithic local-server scenario passes, including
  voice and stream flows; the example media environment also passes.
- Ignored Discord backend history/reopen/favorite-preview scenarios pass against
  a disposable local server. No real account or service credentials are used.
- An old Steam unit test built its negative case by replacing Unix separators;
  its path now uses a native sibling PathBuf. Production matching is unchanged.
  All 220 corrected Discord unit tests pass.

Optimized app/helper builds and smoke checks pass. All eight real WASM runtime
cases pass again against the final optimized helper.

## Local installer and recovery

- Product: Enhanced `0.2.0.0` for the exact official Windows x64 Yapfast `0.2.0`.
- Compiled source checkpoint: `b4af44b00718ae6ddbe89fc74e4b0407f9167445`.
- Base metadata: `Yapfast v0.2.0 · Stable · b4af44b0 · 2026-11-22`.
- Artifact: `.release/v0.2.0.0/Yapfast-Enhanced-Installer.exe`.
- Size: 50,776,406 bytes (48.4 MiB).
- Installer SHA-256: `00c66eee3c69fceafa03c2b72a0c73777f25eb910d66903f4856e7786050254a`.
- Isolated install, offline repair of a missing helper, and byte-exact original
  restoration pass. Unrelated files and a second installation remain untouched.
- Check/stop/install refuse both a modified fingerprint and genuine official
  0.3.0 without changing them. There is no automatic downgrade.
- Scoped shutdown tests: 0.676 s graceful, 5.221 s unresponsive, 5.225 s with a
  stuck helper, and 0.043 s already closed. Another installation stays running.
- The Inno installer compiles; its real UI was not installed into the system
  uninstall registry as part of QA. The bundled manager is exercised directly.

## Older-renderer limits and security

The old profile renderer supports display name, avatar, banner and pronouns,
but not the newer profile-card name styles or avatar decorations. Member-list
name colours remain supported. The account's separate username is unavailable.
Newer outline palette roles map to the existing edge/line colours. Newer
upstream keybinds, profile cosmetics, soundboard UI and deep-link registration
are not implied by this backport. Enhanced supplies its own manual/six-hour
release checker; automatic update installation remains unavailable.

This is an unsigned local test build. Payload hashes and permission checks
remain enforced; they do not prove third-party plugin behavior trustworthy.
API 1 plugins still need testing against the particular host features they use.
Live-account FGF and other plugins were not exercised in this QA run. Linux,
Matrix, debug-only and system-registry tests are not claimed to have run.
The 0.3.0/main source, existing installers and installed app remain unchanged.

Private logs, fixture folders and measured outputs are kept in ignored `.local/`.

## Zero-based initial release

Enhanced `0.2.0.0` uses release title **Enhanced 0.2.0.0** and tag `v0.2.0.0`.
Installer: `.release/v0.2.0.0/Yapfast-Enhanced-Installer.exe`
(50,776,406 bytes), SHA-256 `00c66eee3c69fceafa03c2b72a0c73777f25eb910d66903f4856e7786050254a`.
Compiled source: `b4af44b00718ae6ddbe89fc74e4b0407f9167445`. The app reports `Enhanced 0.2.0.0`.

Release parsing and manager tests, native smoke, installation over the previous
Enhanced build, offline repair, exact original restoration and unsupported-binary
refusal pass. Normal account data and the installed app were not used for QA.
