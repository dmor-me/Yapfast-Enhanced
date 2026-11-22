# Changes

## 0.2.0.0 - 2026-11-22

Experimental backport to genuine Yapfast 0.2.0. Not published.

- Integrate the existing sandboxed WebAssembly runtime, native plugin manager,
  permission review, GitHub marketplace and native media browser.
- Add Enhanced settings, release checks, memory/cache tools and measurements.
- Keep 0.2.0 message editing, member lists and favorite-preview APIs; reject late
  results for cancelled edits.
- Wake the idle watchdog after a pending host reply completes, so cleanup is
  scheduled reliably instead of sleeping without a deadline.
- Reuse verified repair/restore archives and bounded, installation-scoped shutdown.
- Validate native settings, real WebAssembly execution and the no-plugin build.
- Build a local Windows test installer and verify offline repair, exact original
  restoration, scoped shutdown and wrong-version refusal.

See [compatibility status](docs/compatibility-0.2.0.md) for validation gates and
older-renderer limitations. This does not add unrelated Yapfast 0.4 features.
