# Releasing the 0.3.0 compatibility branch

This branch targets **Enhanced 0.3.0.0** on official **Yapfast 0.3.0**. Consult the
[validation record](compatibility-0.3.0.md); no public release is implied by a local
build. Do not publish a compatibility claim before its installer gates pass.

## Source and versions

The full application source, plugin API/runtime, SDK, fixtures and manager are
included. Preserve upstream history, license and [attribution](../NOTICE.md).
The product version in `src/update/enhanced_releases.rs` is four numbers; Cargo's
application version remains three-part `0.3.0`. Plugin/API versions are independent.

Run the source guard, documentation and fixture checks before making a clean
local commit. Publish corresponding source before or alongside distributed binaries.
Private `.local/`, `.release/`, build directories and credentials are excluded.

```powershell
python scripts/check-enhanced-source.py
python scripts/check-doc-links
python scripts/check-plugin-fixtures
python -m unittest discover -s testing -p test_enhanced_release.py
```

## Native builds and packaging

Follow [building](building.md) and prepare the pinned **0.153.3** Windows Skia
archive with `scripts/prepare-windows-skia.py`. Never substitute the 0.4.x graphics
archive. Use the locked Rust/native toolchain and set its printed SKIA_BINARIES_URL.

Set `YAPFAST_RELEASE=1` for the shipping app/helper build. Build `yapfast` and
`yapfast-plugin-host` with `--release`; build the standalone manager from
`tools/yapfast-enhanced-manager/Cargo.toml`. Stage their three executables together.
The packager calls `yapfast --version` and rejects development/wrong-base builds.

Package with `scripts/package-enhanced-inplace.py`, supplying `--app-version 0.3.0`,
`--bin-dir`, an output directory, the verified official `--upstream-exe`, and its
`--upstream-sha256` from the compatibility record. Inno Setup 6 creates the installer;
`--no-installer` creates only the matched payload archive for testing.

## Required verification

Use disposable app/data folders, never an installed account profile. Verify the
linked-stack smoke check, real plugin execution/disable, native preview permission
boundaries, tray/idle reattachment, reopened chats and immediate tool persistence.
Test upgrade, offline repair, exact restoration, wrong-base refusal and unrelated
file preservation. Check installer hashes and `SOURCE.txt` against the source commit.

Keep one current installer and matching archive/checksum set per version. Keep
0.3.0 compatibility work on its branch. A source tag or GitHub publication is a
separate explicit action; local builds do not push anything.
