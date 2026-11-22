# Preparing an Enhanced release

The Enhanced repository is configured for
`https://github.com/dmor-me/Yapfast-Enhanced`.
The Marketplace catalog has its own repository at
`https://github.com/dmor-me/yapfast-plugin-registry`.

## Local preparation

Keep one active release folder: `.release/v0.4.1.0/`. It contains
`Yapfast-Enhanced-Installer.exe`, the matching compatibility ZIP, and
`SHA256SUMS.txt`. The `bundle/` folder holds build inputs, not another installer.
Source files remain in Git. Installer outputs remain ignored and belong on
GitHub Releases rather than in the source tree.

This checkout contains the working Enhanced source, its current uncommitted
changes and upstream Git history. Its `main` branch and `origin` remote are
configured for review. `upstream` points to the original Yapfast repository.
Review and commit the imported changes before creating a release tag.

Local `.release/` outputs are development artifacts, not public releases.
Rebuild the reviewed source revision before distributing a matching public
installer. Do not upload a previously built installer merely because its version
number matches.

Open `README.md` in VS Code and press **Ctrl+Shift+V** for a live preview.
An offline browser preview is also provided at `.local/readme-preview/index.html`.
Regenerate it after editing the README:

```powershell
python -m pip install Markdown==3.7
python scripts/preview-readme.py
```

The Python dependency is optional and used only for the browser preview.

## Source review

The lightweight checks require Python 3.11 or newer and Git. They do not use
network access or require a Rust/Windows toolchain. The updater's two public,
upstream test-only signing fixtures remain in the source, pinned to their known
Git blob IDs. Production signing material must never enter the repository.

- Review `git diff` and `git status`, including new Rust and Slint files.
- Keep caches, credentials, account data and local installers in the ignored
  `.local/`, `.release/` and `target/` directories.
- Preserve upstream copyright notices, dependency licenses and vendored patches.
- Run `python scripts/check-enhanced-source.py`, `python scripts/check-doc-links`
  and `python scripts/check-plugin-fixtures`.
- The Source release checks workflow performs lightweight checks on pushes and
  pull requests. Tests retains the native application checks. The obsolete
  upstream Nightly publisher has been removed from this repository.
- Enhanced installer publishing remains manual. No workflow here publishes
  Enhanced releases automatically.

## Version and build

The Enhanced version is defined in `src/update/enhanced_releases.rs`.
It is currently `0.4.1.0`: the first Enhanced revision for Yapfast `0.4.1`.
Use `0.4.1.2` for the next Enhanced update on that base, or `0.5.0.1` for
the first Enhanced build supporting Yapfast `0.5.0`. Each component is a
canonical integer up to 65535; the Enhanced revision starts at 0. The packager
rejects a version whose first three components differ from `--app-version`.
Internal Cargo package versions remain three-part Semantic Versions and do not
define the displayed Enhanced product version.
Existing `0.1.0` builds have a three-part-only checker: install `0.4.1.0`
manually once. The new checker reads both the four-part tags and legacy tags;
account/plugin data and the existing installation identity are preserved.
The packager reads the Enhanced version from that source file for the installer's
Windows version metadata and welcome heading. The `--app-version` argument
selects the supported official Yapfast version shown separately in the wizard
and recorded in the compatibility manifest.
Choose the Enhanced version before building and use the same number in the tag,
for example `v0.4.1.0`.

Set `YAPFAST_RELEASE=1` for the application build: `cargo --release` optimizes
the executable but does not set its Stable channel. The installer packager runs
`yapfast.exe --version` and refuses a Development build, a non-stable channel,
or a base version that differs from `--app-version`. Rebuild before packaging
if this check fails; do not relabel an existing binary.

Follow [the native Windows build instructions](building.md#windows) to prepare
Rust and the pinned native libraries. Build the app and runtime helper together:

```powershell
$env:YAPFAST_RELEASE = '1'
cargo build --release --locked
$env:CARGO_TARGET_DIR = (Resolve-Path target).Path
cargo build --release --locked --manifest-path tools/yapfast-enhanced-manager/Cargo.toml
cargo test --release --locked -p yapfast --bin yapfast ui::plugin_media::tests
target/release/yapfast.exe --smoke-test
```

The manager has its own lock file. All three executable files must be available
under `target/release/` for packaging.

Commit and tag the reviewed source, then build that source revision.
The packaging script embeds the Git revision in `SOURCE.txt`.
Use a clean source checkout for the public package. Enforce this with
`python scripts/check-enhanced-source.py --require-clean` before tagging or
packaging. The check without this flag also supports a local working-tree review.

The native Windows commands above produce a development build. Before public
binary distribution, also satisfy the path-remapping and dependency-notice
requirements in [Building](building.md#windows). The in-place packager does not
collect every external dependency's source or license notices automatically.
Providing complete corresponding source and notices is a separate binary-release
requirement. Preparing this source repository does not certify old local binaries.

Package the supported official Windows x64 Yapfast 0.4.1 executable with
Inno Setup installed:

```powershell
python scripts/package-enhanced-inplace.py --upstream-exe "<path-to-official-yapfast.exe>" --upstream-sha256 2d8363fad4dbd20632248107da67f74c45bb698e4fdcc416023e001fee3e7419 --app-version 0.4.1 --output .release/v0.4.1.0
```

Validate the generated installer on an isolated installation, open the app,
exercise a plugin through the packaged helper, and check restoration.

## Repository publication checklist

1. Review changed and untracked files, preserving upstream history and notices.
2. Confirm `.local/`, `.release/`, nested build folders and credentials stay ignored.
3. Commit the reviewed source, including lock files, vendored patches, SDKs,
   plugin fixtures, schemas and packaging tools. Review the staged diff before
   committing. Do not use a source archive of the old upstream HEAD.
4. Run Source release checks and the relevant native build tests.
5. Create the `v0.4.1.0` tag only after reviewing the final source commit.
6. For a binary release, rebuild that revision, validate installation and
   restoration, and provide corresponding source and dependency notices.

These are publication steps for the maintainer. Local preparation does not push,
tag or publish anything. GitHub's generated source ZIP and tar.gz include committed
files for the selected tag, not uncommitted work or ignored local build files.
The separate marketplace repository must also be published for default browsing
to work. Keep each reviewed catalog entry pinned to an immutable commit.

## GitHub release

Publish the reviewed source and matching tag, then create a draft release
from that tag. Attach the generated files:

- `Yapfast-Enhanced-Installer.exe`
- `yapfast-enhanced-for-<official-sha256>.zip`
- `SHA256SUMS.txt`

GitHub supplies source ZIP and tar.gz downloads for the tag.
Confirm that the published source includes the corresponding source and build
scripts for the distributed binaries.

Use the current version entry in `CHANGELOG.md` for release notes. State the Enhanced version, supported Yapfast version,
Windows architecture, changes and installation instructions.
The current checker accepts stable tags such as `v0.4.1.0`.
It ignores drafts, prereleases and source-only releases and checks for the
expected installer and compatibility package. Publish as stable after validation.

## Installer shutdown

Setup closes only the selected Yapfast installation and its helper. The ready
page warns to finish calls/uploads and save work. Preparing to Install shows
manual tray-quit guidance. A normal quit is requested first; after five seconds
Setup may force-close verified matching executable paths, with a bounded wait
for locks to clear. It skips this work when already closed. Validate graceful,
unresponsive and other-installation fixtures before publishing.
See [Enhanced tools](enhanced-tools.md).

Run the Windows shutdown integration test with a disposable fixture, never your
installed application as the fixture. The test starts hidden processes in its
own temporary folders, including another installation that must stay running:

```powershell
New-Item -ItemType Directory -Force target/shutdown-fixture
rustc --edition 2024 -O -C target-feature=+crt-static testing/windows_shutdown_fixture.rs -o target/shutdown-fixture/yapfast.exe
python testing/test_enhanced_shutdown.py --fixture target/shutdown-fixture/yapfast.exe --manager target/release/yapfast-enhanced-manager.exe
```

See [universal installer publishing requirements](universal-installer.md) for the optional online dispatcher.

## Zero-based initial release

Enhanced `0.4.1.0` uses release title **Enhanced 0.4.1.0** and tag `v0.4.1.0`.
Installer: `.release/v0.4.1.0/Yapfast-Enhanced-Installer.exe`
(62,299,459 bytes), SHA-256 `9c1769f14c307ec0f650ae706c1781d6745c92eea989949851b56ae29640249d`.
Compiled source: `bdaa34c04cf8c80bfdf1fd86596ce536b5b7b16c`. The app reports `Enhanced 0.4.1.0`.

Release parsing and manager tests, native smoke, installation over the previous
Enhanced build, offline repair, exact original restoration and unsupported-binary
refusal pass. Normal account data and the installed app were not used for QA.

## Zero-based initial release

Enhanced `0.4.1.0` uses release title **Enhanced 0.4.1.0** and tag `v0.4.1.0`.
Installer: `.release/v0.4.1.0/Yapfast-Enhanced-Installer.exe`
(62,264,770 bytes), SHA-256 `d7c912705d1956eda8acae65d2cf38a7dc49a9e528a295f9786a1f8ca52d3919`.
Compiled source: `9128aefc85f799685a87502c6f2f1325f6aec124`. The app reports `Enhanced 0.4.1.0`.

Release parsing and manager tests, native smoke, installation over the previous
Enhanced build, offline repair, exact original restoration and unsupported-binary
refusal pass. Normal account data and the installed app were not used for QA.

The main installer also passes an upgrade initialized by the actual previous
manager executable, updating its integrity pin and restoring the exact original.
The universal online installer compiles and passes local selection/integrity
tests; its live GitHub download and handoff remain a post-publication check.
