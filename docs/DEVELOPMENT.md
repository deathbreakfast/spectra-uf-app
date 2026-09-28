# Spectra UF App development map

Paths below are relative to the repository root.

## Crates

| Crate | Path | Start here |
|-------|------|------------|
| `spectra-backend` | [`spectra-backend/`](../spectra-backend/) | Name validation, catalog helpers, live query execution — `cargo doc -p spectra-backend --open` |
| `spectra-app` | [`spectra-app/`](../spectra-app/) | Leptos UI, `SpectraRoutes`, `#[server]` wrappers — `cargo doc -p spectra-app --open` |
| `spectra-uf-app-e2e` | [`spectra-uf-app-e2e/`](../spectra-uf-app-e2e/) | Playwright lab host + boundary contract tests |
| `protected-spectra-host` | [`examples/protected-spectra-host/`](../examples/protected-spectra-host/) | Minimal Axum teaching host (auth + schema index) |

## `spectra-app` source layout

```text
spectra-app/src/
├── lib.rs              # SpectraRoutes, uf_app!, public re-exports, crate docs
├── help_steps/         # Help spotlight tour inventory (ensure_help_steps_linked)
├── server/             # Higgs #[server] fns, permission gate, error mapping
├── pages/              # Route pages (home, schema index/detail, explore)
├── components/         # Private UI (charts, explore, query, schema, tables)
├── layout/             # SpectraLayout shell
├── lazy_routes.rs      # LazyRoute chunks for WASM split
└── explore_time.rs     # Shared time-range helpers (+ unit tests)
```

Import from crate-root re-exports (`use spectra_app::{SpectraRoutes, list_schema_metadata, …}`).
For Help tours, call `ensure_help_steps_linked()` and enable `uf-integrations` `offering-help`.
Mapping and validation helpers live in `spectra-backend`; avoid reaching into
`spectra_app::server::execute_*` unless you extend server functions in this crate.

## Error model

- [`SpectraOpsError`](../spectra-backend/src/ops_error.rs) — typed ops failures in `spectra-backend`
- [`to_server_fn_error`](../spectra-app/src/server/error.rs) — maps into Leptos `ServerFnError` at the server boundary
- [`server_fn_is_permission_denied`](../spectra-app/src/server/error.rs) — UI helper for permission-denied panels

## Verification layers

See [`VERIFICATION.md`](VERIFICATION.md). CI jobs: `fmt`, `clippy`, `test`, `e2e`, `docs`.

## First success paths

1. **Backend contracts only:** `cargo test -p spectra-backend`
2. **UI unit tests:** `cargo test -p spectra-app --features ssr`
3. **Teaching host:** `cargo run -p protected-spectra-host`
4. **Boundary (no browser):** `cargo test -p spectra-uf-app-e2e --features ssr --test boundary_contract`
5. **Full E2E:** `cargo leptos end-to-end --project spectra-uf-app-e2e`
