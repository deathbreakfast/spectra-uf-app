# spectra-uf-app verification

Re-run after code or doc changes. This workspace is the Spectra operations app
(`spectra-app` Leptos UI + `spectra-backend` pure server contracts + `spectra-uf-app-e2e`
Playwright host). Layer 1 unit + integration tests cover schema catalog, live query
helpers, and sibling-source UI surface contracts. Layer 2 runs browser E2E against
mem Spectra seed data on port 3200.

## Environment

```bash
export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR=target-spectra-uf-app
```

## Teaching host

Axum oneshot under [`examples/protected-spectra-host`](../examples/protected-spectra-host/).
Copy table + product mount sketches live in that host README.

```bash
cargo check -p protected-spectra-host
cargo run -p protected-spectra-host
```

Success line: `protected_spectra_host: OK — /spectra deny/allow + schema index`.
Hydrate/browser is out of gate for the oneshot (`cargo-leptos` + `wasm32` +
Orbital / `uf-product` belong to a composite product host).

## Layer 1 — Unit + integration (CI)

GitHub Actions (`.github/workflows/ci.yml`) runs **fmt**, **clippy**, **test**,
**e2e**, and **docs**. The **test** job includes backend contracts, spectra-app
SSR unit tests, spectra-uf-app-e2e boundary contract tests, SSR compile checks,
and the teaching host run.

Sibling-source UI contracts (no full Orbital compile for needles):

```bash
cargo test -p spectra-backend --test workspace_members --test product_surface
```

Backend + app SSR unit tests:

```bash
cargo fmt -p spectra-backend -p spectra-app -p protected-spectra-host -- --check
cargo clippy -p spectra-backend --all-targets -- -D warnings
cargo clippy -p protected-spectra-host --all-targets -- -D warnings
cargo test -p spectra-backend
cargo test -p spectra-app --features ssr
cargo test -p spectra-uf-app-e2e --features ssr --test boundary_contract
cargo check -p spectra-app --features ssr
cargo check -p spectra-uf-app-e2e --features ssr
```

`cargo fmt --all` can fail when a sibling checkout sits outside this workspace;
package-scoped fmt is the honest local gate.

Full workspace (includes `spectra-app` UI). May fail when the sibling
`uf-product` / `uf-integrations` UI graph does not compile — that is a
host-product UI issue, not a Spectra backend contract gap.
Surface needles for routes, nav testids, `RequireAuthenticated`, and
`QueryTable` live in `product_surface`.

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
# Host-aligned SSR surface (when UI graph compiles):
cargo test -p spectra-app --features ssr
```

### leptos-lints (CI job `leptos-lints`)

Needs `cargo-dylint` / `dylint-link` 6.0.1 and toolchain `nightly-2025-05-14`
(see `leptos-lints@v0.1.2`). Workspace `[workspace.metadata.dylint]` pins the
library; rustc deny names are declared under `[workspace.lints.rust]`.
GitHub Actions runs the same command.

```bash
# cargo install cargo-dylint --locked --version 6.0.1
# cargo install dylint-link --locked --version 6.0.1
# rustup toolchain install nightly-2025-05-14 --component rustc-dev,llvm-tools-preview

export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR=target-spectra-uf-app
export CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback
export RUSTFLAGS="-D warnings -Zcrate-attr=feature(stdarch_x86_avx512)"

cargo dylint --all -p spectra-app --no-deps -- --features hydrate
```

Hard CI job deferred: `spectra-app` hydrate dylint and required clippy still
depend on the Orbital / host graph (same pin risk as full workspace clippy).
CI runs `cargo clippy -p spectra-app --features ssr` as a **non-blocking**
signal until warnings are zero — run locally when the graph is green.

## Layer 2 — Playwright E2E (IsolatedLab)

Host crate: [`spectra-uf-app-e2e`](../spectra-uf-app-e2e/) on `127.0.0.1:3200`. Scenario IDs:
[`spectra-uf-app-e2e/README.md`](../spectra-uf-app-e2e/README.md).

GitHub Actions job **`e2e`** runs the same gate as local:

```bash
export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR=target-spectra-uf-app
cargo leptos end-to-end --project spectra-uf-app-e2e
```

Do not interrupt the run — it exits when Playwright finishes.

Specs cover auth gate, home dashboard, schema list/detail, event/metric explore,
and Help spotlight tours (`help_spotlight.spec.ts`: skip-by-default + all-routes green path)
with mem Spectra seed data (`POST /api/test/seed-data`).

Event explore chart scenarios (validating — assert headline totals / SVG marks /
EmptyState classification, not viewport smoke):

- `pw-spectra-event-view-timeseries-data-happy` / `pw-spectra-event-view-line-chart-data-happy`
- `pw-spectra-event-view-bar-chart-data-happy` / `pw-spectra-event-view-pie-chart-data-happy`
- `pw-spectra-event-aggregate-sum-data-happy`
- `pw-spectra-event-view-pie-need-groupby-sad` / `pw-spectra-event-view-bar-need-groupby-sad`
- `pw-spectra-event-view-timeseries-empty-table-sad`
- `pw-spectra-event-view-pie-unknown-groupby-sad`
- `pw-spectra-event-explore-permission-denied-sad`

Layer 1 still holds stub-shape regression tests (`empty_*` helpers) and live-query
helper contracts (`query_live_contract.rs`), including hardened
`execute_event_aggregate_group_by_slices_happy_path` (Slices required).

Covering integ tests (Layer 1):

- `schema_metadata_list_returns_valid_items_happy_path` / `schema_metadata_detail_unknown_name_is_none_sad`
- `schema_metadata_detail_matches_list_entry_happy_path`
- `execute_event_query_empty_table_happy_path` / `execute_metrics_query_empty_happy_path`
- `dashboard_catalog_summary_counts_happy_path`
- `empty_metrics_query_result_shape_happy_path` / `empty_event_query_result_unknown_table_happy_path`
- `empty_event_aggregate_result_timeseries_stub_happy_path`
- `validate_spectra_query_name_accepts_table_happy_path` / `validate_spectra_query_name_rejects_blank_sad`
- `spectra_query_permission_name_formats_table_happy_path`
- `spectra_product_workspace_members_happy_path`
- `spectra_routes_mount_happy_path` / `layout_auth_gate_and_nav_happy_path` / `ops_reads_require_query_table_happy_path`

## Layer 3 — Cloud + performance

**Waived.** This application workspace; no cloud resources or Criterion benches.
Correctness is in-process against the Spectra schema registry, live router helpers,
and Playwright E2E on the lab host.

## Rustdoc policy

Preferred deny gate (no UI graph):

```bash
RUSTDOCFLAGS="-D rustdoc::broken-intra-doc-links" cargo doc -p spectra-backend --no-deps
```

Workspace `rustdoc::broken_intra_doc_links` is `allow` in `Cargo.toml` because
sibling/cfg-gated links often fail under `--no-deps`. Prefer the
`RUSTDOCFLAGS` deny form above for the backend contract crate. `spectra-app`
rustdoc with deny flags is pin-dependent on Orbital / host graphs.
`spectra-app` still uses `#![allow(missing_docs)]` on macro-heavy UI surfaces.

## Notes

- Prefer `cargo test -p spectra-backend` for backend contract CI when the UI
  dependency graph (`uf-product` via `uf-integrations` / `lepton-shell`) fails to
  compile — report that separately from Spectra contract results.
- Tests may `unwrap`/`expect`; production server fns map failures to
  [`SpectraOpsError`](../spectra-backend/src/ops_error.rs) then `ServerFnError`
  via [`to_server_fn_error`](../spectra-app/src/server/error.rs) (no ordinary-path unwrap).
- Sad-path assertions check message content or `None` — stronger than `is_err()` alone.
- Happy-path tests are named `*_happy_path` so audits detect them.
- `SpectraRoutes` data loaders call the `#[server]` fns; those fns are thin Higgs
  wrappers over `spectra-backend` helpers covered by Layer 1.
