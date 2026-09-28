# Spectra UF App

[![CI](https://github.com/unified-field-dev/spectra-uf-app/actions/workflows/ci.yml/badge.svg)](https://github.com/unified-field-dev/spectra-uf-app/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[GitHub](https://github.com/unified-field-dev/spectra-uf-app) · `cargo doc -p spectra-backend --open`

## About

Spectra UF App is the Unified Field **operations UI** for Spectra schemas,
events, and metrics under `/spectra`. Spectra itself has no built-in UI; hosts
mount this crate so operators can browse registered schemas and explore
event/metric data.

- **UI (`spectra-app`)** — pages, Higgs `#[server]` wrappers, `SpectraRoutes`,
  `uf_app!` registration
- **Backend (`spectra-backend`)** — pure schema/query helpers and [`SpectraOpsError`](spectra-backend/src/ops_error.rs); primary contract CI surface

Repository map and contributor paths: [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md).

Hosts supply Spectra query backends and auth guard context. Enable `ssr` /
hydrate to match your host. Crate-root rustdoc owns Concern → route → server fn
tables; prefer `cargo doc -p spectra-backend --open` for the mapping contract.
UI rustdoc is pin-dependent on Orbital / host graphs. Explore queries currently
return empty stub payloads until a host injects a live Spectra backend.

## Getting started

```toml
[dependencies]
# Pin tag or rev — do not use branch = "main".
spectra-app = { git = "https://github.com/unified-field-dev/spectra-uf-app", package = "spectra-app", rev = "REPLACE_WITH_PIN", default-features = false }
spectra-backend = { git = "https://github.com/unified-field-dev/spectra-uf-app", package = "spectra-backend", rev = "REPLACE_WITH_PIN" }
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

Wire Spectra query backends + session extractors in host bootstrap, then mount
the routes above. Full Leptos SSR hosts live outside this repository; use the
local teaching host for the auth + schema index contract.

```bash
export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR=target-spectra-uf-app
cargo test -p spectra-backend
```

## Workspace

| Crate | Role |
|-------|------|
| [`spectra-app`](spectra-app/) | Leptos ops UI + `SpectraRoutes` + app registration |
| [`spectra-backend`](spectra-backend/) | Pure schema catalog + explore-query helpers + [`SpectraOpsError`](spectra-backend/src/ops_error.rs) |
| [`spectra-uf-app-e2e`](spectra-uf-app-e2e/) | Playwright lab host + SSR boundary contract tests |
| [`protected-spectra-host`](examples/protected-spectra-host/) | Teaching host: deny/allow + schema index |

## Examples

| Host | When to use | Command | Success | Look next |
|------|-------------|---------|---------|-----------|
| [`protected-spectra-host`](examples/protected-spectra-host/) | Auth + `/spectra` schema index | `CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=target-spectra-uf-app cargo run -p protected-spectra-host` | Deny/allow + schema JSON | Mount `SpectraRoutes` |

Copy table + product mount `Cargo.toml`:
[`examples/protected-spectra-host/README.md`](examples/protected-spectra-host/README.md).
More examples: [`examples/README.md`](examples/README.md).

## Security

Auth-gated `/spectra` routes (`QueryTable` plus per-table
`spectra.query.{table}` for explore) and private vulnerability reporting:
[`SECURITY.md`](SECURITY.md). Report vulnerabilities privately — do not open a
public issue for security-sensitive reports.

## Verify

GitHub Actions (`.github/workflows/ci.yml`) runs five jobs aligned with
[`docs/VERIFICATION.md`](docs/VERIFICATION.md): **fmt**, **clippy** (backend +
teaching host; spectra-app SSR tracked non-blocking), **test** (backend contracts,
spectra-app SSR unit tests, e2e boundary contract, SSR compile check, teaching
host run), **e2e** (Playwright via `cargo leptos end-to-end`), and **docs**
(spectra-backend rustdoc with broken-link deny).

```bash
export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR=target-spectra-uf-app
export RUSTFLAGS="-D warnings"
cargo fmt -p spectra-backend -p spectra-app -p protected-spectra-host -- --check
cargo clippy -p spectra-backend --all-targets -- -D warnings
cargo clippy -p protected-spectra-host --all-targets -- -D warnings
cargo test -p spectra-backend --test workspace_members --test product_surface
cargo test -p spectra-backend
cargo test -p spectra-app --features ssr
cargo test -p spectra-uf-app-e2e --features ssr --test boundary_contract
cargo check -p spectra-app --features ssr
cargo check -p spectra-uf-app-e2e --features ssr
cargo check -p protected-spectra-host
cargo run -p protected-spectra-host
RUSTDOCFLAGS="-D rustdoc::broken-intra-doc-links" cargo doc -p spectra-backend --no-deps
# Layer 2 (local or CI e2e job):
cargo leptos end-to-end --project spectra-uf-app-e2e
```

Teaching host success line:
`protected_spectra_host: OK — /spectra deny/allow + schema index`.
Full command block: [`docs/VERIFICATION.md`](docs/VERIFICATION.md). Contribute:
[`CONTRIBUTING.md`](CONTRIBUTING.md).

## FAQ

**Is this a standalone Spectra server?** No. `spectra-app` mounts under a host
`<Routes>` tree. Schema registry and query storage live in the Spectra core
crates; hosts inject live query backends.

**Why is there a separate `spectra-backend` crate?** So schema catalog and
explore-query stub helpers stay unit-testable without the Leptos/UI dependency
graph. `spectra-app` `#[server]` fns are thin wrappers over those helpers.

**What can operators do from the UI?** Browse registered schemas and explore
events/metrics (table, time series, breakdowns). Explore payloads are empty stubs
until the host wires a live Spectra backend.

**Where does Spectra core fit?** Schema registration and storage live in the
Spectra core repos. This repo maps admin list/get/query APIs into UF ops pages.

## License

MIT. See [LICENSE](LICENSE), [CONTRIBUTING.md](CONTRIBUTING.md),
[SECURITY.md](SECURITY.md), and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
