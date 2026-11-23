# Changes

## 0.3.0.0 - 2026-11-22

Experimental backport to genuine Yapfast 0.3.0. Not published.

- Integrate the existing sandboxed WebAssembly runtime, native plugin manager,
  permission review, GitHub marketplace and native media browser.
- Add Enhanced settings, release checks, memory/cache tools and measurements.
- Keep 0.3.0 message editing, member lists and favorite-preview APIs; reject late
  results for cancelled edits.
- Wake the idle watchdog after a pending host reply completes, so cleanup is
  scheduled reliably instead of sleeping without a deadline.
- Reuse verified repair/restore archives and bounded, installation-scoped shutdown.
- Build a local Windows test installer; verify real WASM execution, offline repair,
  exact original-binary restoration and incompatible-binary refusal.

See [compatibility status](docs/compatibility-0.3.0.md) for validation gates and
older-renderer limitations. This does not add unrelated Yapfast 0.4 features.
