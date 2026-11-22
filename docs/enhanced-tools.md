# Enhanced settings and tools

Enhanced settings sit between Plugins and Advanced. The version card identifies
the Enhanced version, supported upstream version and Enhanced release date.
Copy version includes all of that information.

## Release checks

Check now reads stable Windows releases from dmor-me/Yapfast-Enhanced on GitHub.
The optional Check for updates preference is off by default and is saved per
app data directory. When enabled, it checks at startup and every six hours.
Manual and periodic checks share the release parser, HTTP timeout, response-size
limit and compatible-package checks. The timer continues while the app is in its
tray. Automatic installation remains unavailable.

## Memory

- Release DM memory when switching chats preserves the existing setting.
- Release inactive server history releases closed Discord server timelines.
  The disk snapshot is saved before dropping history, and pending sends and
  uploads remain tracked. Both settings are off by default and take effect
  immediately.
- Media cache budget selects 16, 32, 64, 128 or 256 MiB for the native plugin
  media-browser preview cache. It preserves viewport-based priorities. Other app
  caches, live rows, GPU resources and the helper have independent memory use.
- Release unused previews now drops eligible offscreen plugin previews and
  converted-preview cache references. Backend previews are retained when other
  owners still hold their images. The result reports released cache references;
  the operating system's process-memory reading can change later.

## Disk cleanup

Show cache size scans only discord-pictures, discord-previews, discord-gifs and
discord-gif-home beneath this app data directory. Clean older media asks for
confirmation and removes regular media-cache files older than seven days.
It skips partial downloads, symlinks and directories resolving outside the data
directory. File-removal failures are reported. It preserves account settings,
credentials, history snapshots, plugin data and media still within the age limit.

## Performance information

The panel samples only while it is expanded and Enhanced settings are visible.
Process scans run on a background worker every three seconds. Measurements show
resident process RAM and CPU normalized to the whole logical processor.
The first CPU sample warms up the counter. Shared resident pages may appear in
both app and helper readings.

Plugin rows show actual runtime status and configured WebAssembly memory caps.
Per-plugin allocation telemetry is unavailable through the current host API and
is labeled Unavailable. Native previews and shared runtime overhead have their
own ownership and are included in the appropriate process reading.

## Plugin restart controls

Restart with plugins paused launches once with --no-plugins. Restart with plugins
enabled removes that option. Both preserve the active data-directory argument
and stored plugin settings and enablement. The confirmation explains that a
restart ends active calls.

## Automatic installer shutdown

New Enhanced builds listen for an installation-scoped Windows event. The
installer manager signals it to request a full event-loop exit, including tray
sessions, and waits for the executable lock to clear. Existing builds fall back
to Windows Restart Manager. The request targets installation file paths.
If an older build refuses the graceful Restart Manager request, Windows retries
with forced application closure for that installation. Finish calls, uploads
and unsaved work before continuing in Setup. Other shutdown failures stop
installation with a retry message.
