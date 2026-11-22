# Attribution and source

Yapfast Enhanced extends the native [Yapfast application](https://github.com/yapfast/yapfast).
This source tree retains upstream history and copyright notices.

The plugin implementation derives from
[blockgameentity/yapfast, feat/plugins](https://github.com/blockgameentity/yapfast/tree/feat/plugins),
at commit `cb31511cb33337925a777fec9c05e52399634924`.

The application is licensed under [GPL-3.0-or-later](LICENSE). Vendored libraries,
fonts, emoji graphics and other dependencies retain their original licenses.
Consult their license and notice files in `vendor/`, `ui/fonts/` and the relevant
source directories. This notice does not replace those files.

The Enhanced Ribbon E artwork derives from the original Yapfast mark and uses
the original palette. Its active assets and regeneration instructions are in
[Enhanced branding](docs/branding/README.md).

Release maintainers must provide the source revision and required dependency
notices for the binaries they distribute. A GitHub tag's source archive alone
does not bundle external Cargo/Git dependencies. The release workflow and
[release guide](docs/releasing-enhanced.md) distinguish source publication from
building a matching binary package.

## Technology acknowledgments

Yapfast uses Slint and Skia, LiveKit and WebRTC, matrix-rust-sdk and
DeepFilterNet. Its Discord implementation benefits from the community-maintained
Discord Userdoccers documentation. The upstream authors and dependency notices
remain credited in their source files. Enhanced is not affiliated with or
endorsed by Discord Inc. Discord is a trademark of Discord Inc.
