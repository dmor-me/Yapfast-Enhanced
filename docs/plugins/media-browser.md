> This guide describes the shared Enhanced API 1 interfaces. For this Yapfast 0.1.0 build, see [host availability and limitations](../compatibility-0.1.0.md).

# Native media browser extension (Enhanced v1)

This surface is a bounded native view for a Wasm plugin's media collections. The guest owns collections and actions; the host owns Slint controls, decoding, network grants and native file dialogs. It extends the existing Extism runtime and `ui.open_dialog` tree rather than adding a second plugin engine.

`ui.features` returns `{ "media_browser": 1, "gif_favorites": 1, "json_files": 1 }`. Guests should check it before drawing. An older build can reject a manifest's newer capabilities before the guest runs; users must install a compatible Enhanced host first.

## Data access

- `gifs.favorites({offset,limit})` needs `media.read`. Returns `{loaded,total,offset,items}`. Items contain URL, preview source, video flag, order and dimensions. Pages cap at 200. `loaded:false` asks the guest to retry after the host loads favorites; errors/timeouts must be shown. The host supplies no credentials. Reads cap the offset at 10,000.
- `files.open_text({})` needs `files.open` and a recent gesture. A native chooser returns a UTF-8 string or null on cancel, up to 900 KB and the encoded transport cap.
- `files.save_text({name,text})` needs `files.save` and a recent gesture. A safe .json basename and at most 900 KB are required. The host owns the selected path and uses an atomic temporary-file replacement. Returns true or null on cancel. Account/session changes discard stale results.

Host calls return a pending tag immediately. Real results arrive in `Context::results()`/`Plugin::answered`; guests must acknowledge writes and handle denial/error. The Rust PDK's new default `Plugin::batch` callback also receives empty timer wake batches without changing existing plugins.

## Native surface

Draw `{tree:{media_browser:{...}}}` with `ui.open_dialog`. The contract is `crates/yapfast-plugins/src/media_browser.rs`; unknown fields are rejected. Collections, navigation rows, cards, folder/category choices, paging, selection, forms, confirmation and errors are ordinary data. Pages cap at 200 cards; choices/navigation and text are bounded independently. Media browsers are accepted only in the dialog slot, behind `ui.modify`.

`ui/plugin_media_browser.slint` renders the themed surface. Existing `yf_input` widget events carry action IDs and values. Dialogs and the `gif_picker` button slot receive the currently selected conversation as subject. The additional slot is appended to preserve the existing indexes. Runtime owners are registered only from enabled, verified installed manifests; disabled/uninstalled late paints are discarded.

Native previews have a dedicated read-only media route through the existing NetworkGate (declarations, grants, HTTPS, private-address and redirect checks), never an arbitrary GUI HTTP client. The native loader accepts curated Discord attachment and external-preview paths, including the numbered Discord image proxy. An exact preview URL from the current account's favorites can use Yapfast's existing on-disk favorite thumbnail path under media.read. This draws into the native gallery without exposing the thumbnail file or its bytes to the plugin. Guest http.fetch remains blocked from every Discord host. Every redirect and DNS result is checked again. The native loader caps concurrency at four. Its general preview fetch caps responses at 4 MiB and requests at 20 seconds. The favorite-only route shares the built-in picker's bounded thumbnail generation and has a 20-second timeout before the normal route. Cache limits are 64 entries/64 MiB of retained source and still-image accounting, with native bounded lazy animation models. This is not an exact process/GPU-memory ceiling: decoders, models and renderer surfaces add transient overhead. No browser or webview is involved. Failed previews show a reason. Extra preview hosts use the existing consent dialog.

## Current verification

Permission tests cover missing capabilities, native chooser gesture requirements, path/size rejection, and bounded media trees. Application tests cover favorite paging and runtime owner registration without developer folders, including disabled-owner late paint rejection. The Favorite GIF Folders plugin provides a real-helper integration test using 2,400 simulated favorites, import/export, failure injection and account isolation. That harness does not exercise a live Discord account or OS chooser interaction.

Windows app-owned helper commands use CREATE_NO_WINDOW for runtime, fetch, and catalog launches, preserving their stdio pipes and error logs. Closing Yapfast still owns the helper lifecycle; no independent service is installed.
