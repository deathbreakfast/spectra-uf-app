# spectra-app

Leptos operations UI for Spectra: schema browsing, event/metric explore, and a callsite graph under
`/spectra`.

## Quickstart

1. **Mount routes** — nest [`SpectraRoutes`] in the host `<Routes>` tree with `ssr` enabled.
2. **Wire backends** — host bootstrap installs Spectra query backends and session extractors.
3. **Verify** — teaching host (`cargo run -p protected-spectra-host`) or full E2E
   (`cargo leptos end-to-end --project spectra-uf-app-e2e`).

```toml
# Pin tag or rev — do not use branch = "main".
spectra-app = { git = "https://github.com/unified-field-dev/spectra-uf-app", package = "spectra-app", rev = "REPLACE_WITH_PIN", default-features = false }
```

```rust,ignore
use spectra_app::SpectraRoutes;
use leptos_router::components::Routes;

view! {
    <Routes fallback=|| "not found">
        <SpectraRoutes />
    </Routes>
}
```

Compose into a host that supplies Spectra query backends and the auth/context
extractors the app expects. Enable `ssr` / `hydrate` to match your host. For
Help spotlight tours, enable `uf-integrations` `offering-help` (or `full`) and
call `spectra_app::ensure_help_steps_linked()` (also called from `SpectraRoutes`).

## Where to look

| Topic | Location |
|-------|----------|
| Crate docs (routes, server fns, examples) | `cargo doc -p spectra-app --open` — [`src/lib.rs`](src/lib.rs) |
| Help spotlight inventory | [`src/help_steps/`](src/help_steps/) |
| Server functions + errors | [`src/server/`](src/server/) |
| Mapping / validation contracts | [`spectra-backend`](../spectra-backend/) |
| Contributor map | [`../docs/DEVELOPMENT.md`](../docs/DEVELOPMENT.md) |
| CI / verify layers | [`../docs/VERIFICATION.md`](../docs/VERIFICATION.md) |

Prefer crate-root re-exports (`use spectra_app::{SpectraRoutes, list_schema_metadata, …}`).
Import mapping helpers from `spectra-backend`, not via `spectra_app::server::execute_*`.

[`SpectraRoutes`]: https://docs.rs/spectra-app/latest/spectra_app/fn.SpectraRoutes.html
