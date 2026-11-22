# Enhanced settings and tools

On this branch, Enhanced 0.2.0.0 extends genuine Yapfast 0.2.0. The **Enhanced**
settings area sits between **Plugins** and **Advanced**, using the Ribbon E icon.
See [compatibility limits](compatibility-0.2.0.md) for older-renderer differences.

The version card supports Copy version. The page checks the Enhanced GitHub
release channel manually or every six hours when enabled. Links contains the
Enhanced GitHub repository. Automatic updates remain unavailable.

## Memory and cache controls

- **Release DM memory when switching chats** and **Release inactive server
  history** are optional and off by default. Changes apply immediately and
  persist. Leaving a conversation releases its eligible backend/UI history and
  previews while preserving disk cache, unread state and pending sends/uploads.
- **Release unused previews now** drops eligible RAM references. Active views
  retain the pictures they need; cached disk files stay available.
- **Media cache budget** limits retained decoded previews. A lower budget can
  require more reloading. It does not limit the whole application or guest memory.
- **Local cache cleanup** measures known media-cache folders and can remove
  eligible files at least seven days old. This recovers disk space, independently
  of RAM cleanup. It does not remove accounts, message history or plugin data.

## Troubleshooting and measurements

**Restart with plugins paused** keeps installed/enabled choices while starting
with `--no-plugins`. The in-app restart-with-plugins-enabled action restores normal
operation. These are restart-based actions, not a promise of immediate hot resume.

Performance information separates application and helper process memory/CPU.
Guest memory reports describe WebAssembly memory, not an exact attribution of all
shared engine code, allocator overhead or CPU to individual plugins.

Plugin storage totals include files, saved data and rollback files. The shared
compilation cache is shown separately, without adding it to every plugin total.
All displayed MiB amounts use 1 MiB = 1,048,576 bytes; disk size is distinct from RAM.
