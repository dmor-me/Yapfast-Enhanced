# Experimental Enhanced module loader

This is an isolated feasibility prototype, separate from the current Enhanced
installer. It loads a DLL into a newly launched official Yapfast preview process
without changing `yapfast.exe` on disk.

## What it proves

- A separate native module can attach through a Windows API scoped to one new
  Yapfast UI thread, without private Rust addresses or Slint object layouts.
- It creates a Win32 child toolbar attached over the application window.
- Its button runs the existing capability-free Ping WebAssembly plugin through
  the existing Extism helper and displays `pong`.
- The same DLL can be tested across official versions without recompilation.

The toolbar is an overlay, not an element in the compiled Slint composer.
The module does not currently provide Yapfast chat events, media/favorites access,
message sending, history release, Enhanced settings or Favorite GIF Folders.
Those capabilities require a real host bridge and are explicitly unavailable.
This prototype does not establish that private backend hooks can stay compatible
across versions, or that this design already needs less maintenance.

## Build

Use Rust 1.97.1 and a Windows x64 MSVC developer environment. From the repository
root:

```powershell
cargo build --locked --release --manifest-path tools/yapfast-enhanced-loader/Cargo.toml
cargo test --locked --release --manifest-path tools/yapfast-enhanced-loader/Cargo.toml
```

The standalone crate has its own lock file and is not a default workspace
member. Its output is `target/release/yapfast-enhanced-loader.exe` and
`target/release/yapfast_enhanced_module.dll`, unless `CARGO_TARGET_DIR` is set.
It depends on the existing `yapfast-plugin-api` crate. Build or supply the
existing `yapfast-plugins.exe` runtime helper separately.

## One-click local demo

In the prepared development checkout, double-click `Test-Enhanced-Loader.cmd`.
It uses the isolated, hash-verified official 0.4.1 test copy and creates a fresh
sample profile. Ping runs once automatically, then the button can be clicked
again. A successful response says `Ping: pong`. The demo leaves the preview open
for inspection. Use the preview app's tray menu to Quit when finished if the
window close button only hides it. This convenience launcher requires the local
prepared binaries and test copy, which are not part of a source checkout.

## Test an official executable

Keep the loader EXE and DLL together. The app must be an explicit `yapfast.exe`.
The launcher only creates a new `--preview` process with a new data directory.
It never attaches to an existing app, installs files into an app folder, changes
shortcuts or registers anything for startup.

```powershell
target/release/yapfast-enhanced-loader.exe `
  --app "C:/isolated-official-app/yapfast.exe" `
  --runtime-helper "C:/tested-runtime/yapfast-plugins.exe" `
  --plugin-dir plugins/ping `
  --state-dir .local/loader-test/new-session `
  --self-test --close-after-test
```

Omit `--close-after-test` to inspect the preview window. Existing state directories
are refused to protect normal profiles. `loader-report.json` records the exact app
and DLL hashes, whether the app file remained unchanged and whether the test
passed. `module-ready.json` lists the limited capabilities actually provided.
`ping-result.json` records the real runtime result. A failed run reports an error
and closes only the preview child it started. Test cleanup can terminate that
child if closing to tray leaves it alive.

## Boundaries and lifecycle

The Windows hook is thread-specific, never global. The callback does not inspect
or record keyboard/message contents. The module has no DllMain work. Once it
registers window procedures, it pins itself for the target process lifetime to
avoid dangling callbacks if the launcher exits. Restart the preview app to unload
or replace it. Nothing persists in the official executable or Windows configuration.

The helper uses the existing bounded plugin framing. Runtime and module hashes
are checked for this launch, and privileged plugin host calls are denied. Only
the shipped capability-free Ping manifest is accepted. This is not a general
plugin installer or an independently audited isolation boundary.

## Compatibility results

Tested on Windows 11 x64 on 2026-11-18. The same compiled DLL and existing
Extism helper passed on both official executables. Each run attached the child
toolbar, invoked the shipped Ping plugin and returned `pong`, then closed its
isolated preview process. App executable hashes matched before and after each run.

| Official app | Executable SHA-256 | Result |
| --- | --- | --- |
| Yapfast 0.4.0 x64 | `32d75df588581a0120157ab15d379599e3c9a98ac978e62f63a2b5fb75c3e69c` | Overlay and Ping passed |
| Yapfast 0.4.1 x64 | `2d8363fad4dbd20632248107da67f74c45bb698e4fdcc416023e001fee3e7419` | Overlay and Ping passed |

The 0.4.0 portable ZIP came from the official `yapfast/yapfast` GitHub release.
Its SHA-256 matched the release asset digest
`0677ac45cb6fa68a1f62d14e3848dc7ac452604819197d42b53689038cc95680`.
The 0.4.1 input matched the already verified official executable fingerprint.
These checks identify the inputs, not an independent publisher-signing guarantee.

The four loader unit tests cover ABI/session refusal, executable architecture
and image-kind refusal, request/reply matching, and explicit denial of privileged
plugin host calls. Clippy passed with warnings denied. Repeat the actual runtime
tests with `test-compatibility.ps1`; ordinary unit tests do not exercise the UI
or prove compatibility with an official release.

The existing tested module hash is `e72d4799a59ff64b5a1f02091d61e6c34918c872841cf135355eeaae7e50cd5d`. Raw run reports
are local under `.local/loader-feasibility/validated-final/`; they contain test
paths and should not be included in public source. No RAM/CPU performance claim
or future-version compatibility claim follows from these two tests.

## Remaining host integration

The original 0.4.1 executable exposes dependency/entropy exports, not a supported
Enhanced chat/backend interface. Source inspection also found no existing
external backend command channel that supplies our required plugin host APIs.
An OS window hook does not expose Slint models or Rust application state.

Before replacing the current Enhanced installer, the next work must establish
a real bridge for app events, gesture-gated composer actions and media access,
with explicit permission/capability negotiation. Memory tools need backend/UI
ownership hooks as well. Prefer a versioned upstream extension interface if the
project accepts it. Otherwise, per-version native adapters need separate proof
and maintenance. Open source helps identify changed code, but it does not make
private addresses or Rust object layouts stable.

The main application, current production installer and installed user profiles
remain unchanged by this experiment. The prototype is not part of the current
v0.4.1.0 installer and cannot run Favorite GIF Folders yet.
