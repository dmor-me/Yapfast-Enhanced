# Enhanced 0.4.0 compatibility

Branch: `compat/yapfast-0.4.0`. Product: **Enhanced 0.4.0.0**; tag: `v0.4.0.0`.

## Source provenance

The branch starts at official Yapfast `v0.4.0` (`72d78a05`), with Enhanced
integration ported from saved main commit `4f66f33a`. This is a native 0.4.0
build, not a renamed 0.4.1 binary. It preserves the original push-to-talk UI and
hook; the 0.4.1 keybind module and settings page are not introduced.
The existing Extism/WebAssembly runtime, plugin host APIs, native plugin UI,
Enhanced tools and installer/recovery manager are reused.

## Supported official Windows binary

The official [Yapfast 0.4.0 release](https://github.com/yapfast/yapfast/releases/tag/v0.4.0)
portable ZIP is used as the disposable installer fixture.

- ZIP SHA-256: `0677ac45cb6fa68a1f62d14e3848dc7ac452604819197d42b53689038cc95680`.
- Executable SHA-256: `32d75df588581a0120157ab15d379599e3c9a98ac978e62f63a2b5fb75c3e69c`.

Only that exact original executable, or an intact managed Enhanced installation
of this base, is accepted. Compatibility on another base is not assumed.

## Validation

The optimized Windows application and WebAssembly helper build successfully.
Compiled source checkpoint: `fab76d7dbb6190ef1c90de410e29a76fe22d66b1`.
The application reports `Enhanced 0.4.0.0` and `Yapfast v0.4.0 · Stable`.

- 1,481 native tests pass: 393 application/UI, 626 backend, plugin permission,
  installation, marketplace, security, scheduling, API and real helper execution.
- Eight backend integration scenarios execute real WebAssembly plugins,
  including startup, commands, hooks, trapping and shutdown after disabling.
- 44 installer-manager tests and 10 Python release/packaging tests pass.
- WebAssembly fixture integrity, source-publication and documentation links pass.
- 38 environment-dependent tests remain ignored (including live/fake-server,
  audio and performance scenarios); these are not counted as passed.
- Native demo captures show this branch's own version and settings layout.

The notification icon now uses the actual PNG asset instead of assuming the
ICO's bitmap frames contain PNG bytes. The native reopen test uses callbacks
from the newly attached window. Existing security tests are adapted to the
current authoritative enabled-manifest model and supported `files.save`
permission; permission enforcement is not relaxed.

Disposable installation, repeated installation, offline repair, exact original
restoration and unsupported-executable refusal all pass. A genuine 0.4.1 binary
is refused unchanged. Release checksums are in `SHA256SUMS.txt`. No real account
or installed app is used by the integration fixtures. Live-account/manual plugin testing
remains a separate release check. Installers are unsigned.

## Release files

Release title: **Enhanced 0.4.0.0**. Tag: `v0.4.0.0` on this branch, not main.
The version-specific installer is under `.release/v0.4.0.0/`, alongside its exact
fingerprint compatibility ZIP and `SHA256SUMS.txt`. Keep their original names.
An optional source ZIP is prepared from the committed source only.

Publish this compatibility release without marking it Latest; 0.4.1.0 remains
the current main release. The existing universal installer detects 0.4.0 after
these matching release assets are published. It does not need a separate rebuild.
Nothing is pushed, tagged or published by this local preparation.
