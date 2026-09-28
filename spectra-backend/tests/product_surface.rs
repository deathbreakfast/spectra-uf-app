//! Product surface contracts for spectra-app (sibling crate).
//!
//! Lives under `spectra-backend` so CI can gate route/testid/auth/QueryTable needles
//! without compiling Orbital/turf UI when host pins churn. Pattern matches
//! photon-uf-app / boson-uf-app / chronon-uf-app `*-backend/tests/product_surface.rs`,
//! gauge `gauge/tests/product_surface.rs`, and lepton-uf-app
//! `lepton-shell/tests/product_surface.rs`.

use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read_app(rel: &str) -> String {
    let path = workspace_root().join("spectra-app").join("src").join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn spectra_routes_mount_happy_path() {
    let lib = read_app("lib.rs");
    for needle in [
        r#"path!("spectra")"#,
        r#"path!("")"#,
        r#"path!("schema")"#,
        r#"path!("schema/:name")"#,
        r#"path!("schema/:name/explore")"#,
        r#"path!("metric/:name/explore")"#,
        "SpectraLayoutRouteView",
        "id: \"spectra\"",
        "route_path: \"/spectra\"",
        "permission_manifest: permissions::SpectraPermission",
    ] {
        assert!(
            lib.contains(needle),
            "SpectraRoutes / uf_app missing `{needle}`"
        );
    }
}

#[test]
fn spectra_routes_drop_leaf_sad_path() {
    let lib = read_app("lib.rs");
    for needle in [
        r#"path!("schema")"#,
        r#"path!("schema/:name")"#,
        r#"path!("schema/:name/explore")"#,
        r#"path!("metric/:name/explore")"#,
    ] {
        assert!(
            lib.contains(needle),
            "removing `{needle}` drops a Spectra ops funnel entry"
        );
    }
    assert!(
        !lib.contains("unimplemented!"),
        "SpectraRoutes must not ship unimplemented placeholders"
    );
}

#[test]
fn uf_app_wrong_id_sad_path() {
    let lib = read_app("lib.rs");
    assert!(
        lib.contains("id: \"spectra\""),
        "wrong uf_app id breaks Orbital host registration"
    );
    assert!(
        !lib.contains("id: \"spectra-app\""),
        "uf_app id must stay `spectra` (product route id), not crate name spectra-app"
    );
}

#[test]
fn layout_auth_gate_and_nav_happy_path() {
    let layout = read_app("layout/spectra_layout.rs");
    for needle in [
        "spectra-app-root",
        "RequireAuthenticated",
        "permission_name=\"QueryTable\"",
        "Outlet",
        "nav-spectra-home",
        "nav-spectra-schemas",
        "AppBarUserMenu",
        "UnifiedFieldShellLayout",
        "Breadcrumb",
        "BreadcrumbButton",
        "Link href",
    ] {
        assert!(
            layout.contains(needle),
            "SpectraLayout missing contract `{needle}`"
        );
    }
}

#[test]
fn layout_drop_auth_guard_sad_path() {
    let layout = read_app("layout/spectra_layout.rs");
    assert!(
        layout.contains("RequireAuthenticated") && layout.contains("<Outlet />"),
        "removing RequireAuthenticated opens /spectra pages to anonymous sessions"
    );
}

#[test]
fn layout_missing_nav_sad_path() {
    let layout = read_app("layout/spectra_layout.rs");
    assert!(
        layout.contains("nav-spectra-schemas"),
        "dropping nav-spectra-schemas breaks operator left-nav contract"
    );
}

fn read_server_sources() -> String {
    [
        read_app("server/mod.rs"),
        read_app("server/dashboard.rs"),
        read_app("server/permissions.rs"),
    ]
    .join("\n")
}

#[test]
fn ops_reads_require_query_table_happy_path() {
    let server = read_server_sources();
    for fn_name in [
        "list_schema_metadata",
        "get_schema_metadata",
        "get_spectra_dashboard_summary",
        "query_metrics",
        "query_events",
        "query_event_aggregate",
    ] {
        assert!(server.contains(fn_name), "server missing `{fn_name}`");
    }
    assert!(
        server.contains("SpectraRouter::try_global"),
        "explore server fns must read the installed global Spectra router"
    );
    assert!(
        server.contains("execute_event_query"),
        "query_events must delegate to spectra-backend live helper"
    );
    let query_attr = r#"permission = "QueryTable""#;
    assert!(
        server.matches(query_attr).count() >= 6,
        "catalog + explore server fns must carry QueryTable permission attribute"
    );
    assert!(
        server.contains("SPECTRA_QUERY_PERMISSION: &str = \"QueryTable\""),
        "SPECTRA_QUERY_PERMISSION constant must stay QueryTable"
    );
}

#[test]
fn ops_reads_drop_query_table_sad_path() {
    let server = read_server_sources();
    let query_attr = r#"permission = "QueryTable""#;
    assert!(
        server.matches(query_attr).count() >= 6,
        "dropping QueryTable from any server fn opens Spectra ops without the query gate"
    );
    assert!(
        !server.contains(r#"permission = "GaugeAdmin""#)
            && !server.contains(r#"permission = "PhotonAdmin""#)
            && !server.contains(r#"permission = "BosonAdmin""#)
            && !server.contains(r#"permission = "ChrononAdmin""#),
        "Spectra ops must not gate on GaugeAdmin / PhotonAdmin / BosonAdmin / ChrononAdmin"
    );
}

#[test]
fn server_require_session_happy_path() {
    let server = read_app("server/mod.rs");
    assert!(
        server.contains("fn require_session")
            && (server.contains("Authentication is required")
                || server.contains("SpectraOpsError::AuthRequired"))
            && server.contains("session_user_id()"),
        "server must fail closed without a session"
    );

    let perms = read_app("server/permissions.rs");
    assert!(
        perms.contains("fn require_spectra_query")
            && perms.contains("SpectraOpsError::PermissionDenied")
            && perms.contains("spectra_query_permission_name")
            && perms.contains("fn ensure_table_query_permissions"),
        "require_spectra_query must deny when Gauge spectra.query.{{table}} is missing; ensure_table_query_permissions must stay for host boot sync"
    );

    for call_site in [
        "list_schema_metadata",
        "get_schema_metadata",
        "query_events",
        "query_metrics",
        "query_event_aggregate",
    ] {
        assert!(server.contains(call_site), "server missing `{call_site}`");
    }
}

#[test]
fn server_drop_require_session_on_list_and_query_sad_path() {
    let server = read_app("server/mod.rs");

    let start = server
        .find("pub async fn list_schema_metadata")
        .expect("list_schema_metadata");
    let body = &server[start..start + 350.min(server.len() - start)];
    assert!(
        body.contains("require_session(&ctx)?"),
        "list_schema_metadata must call require_session before Spectra IO"
    );

    let start = server
        .find("pub async fn query_events")
        .expect("query_events");
    let body = &server[start..start + 450.min(server.len() - start)];
    assert!(
        body.contains("require_session(&ctx)?"),
        "query_events must call require_session before Spectra IO"
    );
    assert!(
        body.contains("require_spectra_query"),
        "query_events must call require_spectra_query before the explore stub"
    );
}

#[test]
fn explore_require_spectra_query_happy_path() {
    let server = read_app("server/mod.rs");
    for (fn_name, arg_needle) in [
        ("query_metrics", "require_spectra_query(&query.metric)"),
        ("query_events", "require_spectra_query(&query.table)"),
        (
            "query_event_aggregate",
            "require_spectra_query(&request.table)",
        ),
    ] {
        let start = server
            .find(&format!("pub async fn {fn_name}"))
            .unwrap_or_else(|| panic!("{fn_name}"));
        let body = &server[start..start + 500.min(server.len() - start)];
        assert!(
            body.contains(arg_needle),
            "{fn_name} must gate explore with `{arg_needle}`"
        );
    }
}

#[test]
fn explore_drop_require_spectra_query_sad_path() {
    let server = read_app("server/mod.rs");
    let start = server
        .find("pub async fn query_metrics")
        .expect("query_metrics");
    let body = &server[start..start + 500.min(server.len() - start)];
    assert!(
        body.contains("require_spectra_query(&query.metric)"),
        "dropping require_spectra_query on query_metrics skips per-table Gauge gate"
    );
    let start = server
        .find("pub async fn query_event_aggregate")
        .expect("query_event_aggregate");
    let body = &server[start..start + 500.min(server.len() - start)];
    assert!(
        body.contains("require_spectra_query(&request.table)"),
        "dropping require_spectra_query on query_event_aggregate skips per-table Gauge gate"
    );
}

#[test]
fn index_pages_testid_and_list_bindings_happy_path() {
    let home = read_app("pages/home.rs");
    assert!(
        home.contains("spectra-home-page"),
        "SpectraHomePage missing spectra-home-page testid"
    );
    let detail = read_app("pages/schema_detail/mod.rs");
    assert!(
        detail.contains("spectra-schema-detail-page"),
        "SchemaDetailPage missing spectra-schema-detail-page testid"
    );
    let index = read_app("pages/schema_index/mod.rs");
    assert!(
        index.contains("schema-index-page"),
        "SchemaIndexPage missing schema-index-page testid"
    );
    let section = read_app("pages/schema_index/components/schema_index_section.rs");
    assert!(
        section.contains("list_schema_metadata"),
        "SchemaIndexSection must bind list_schema_metadata"
    );

    let event = read_app("pages/event_explore/mod.rs");
    assert!(
        event.contains("spectra-event-explore-panel"),
        "EventExplorePage missing spectra-event-explore-panel testid"
    );
    let event_panel = read_app("pages/event_explore/components/event_explore_panel.rs");
    for needle in ["query_events", "query_event_aggregate"] {
        assert!(
            event_panel.contains(needle),
            "EventExplorePanel missing `{needle}`"
        );
    }

    let metric = read_app("pages/metric_explore/mod.rs");
    assert!(
        metric.contains("spectra-metric-explore-panel"),
        "MetricExplorePage missing spectra-metric-explore-panel testid"
    );
    let metric_panel = read_app("pages/metric_explore/components/metric_explore_panel.rs");
    assert!(
        metric_panel.contains("query_metrics"),
        "MetricExplorePanel must bind query_metrics"
    );
    assert!(
        !metric.contains("use orbital::components::{ContentContainer")
            && !metric.contains("<ContentContainer")
            && !metric_panel.contains("<ContentContainer"),
        "metric explore canvas must not use ContentContainer gutter"
    );
    assert!(
        !metric.contains("Size240"),
        "metric explore canvas must not use Size240 page gap"
    );

    let event_mod = read_app("pages/event_explore/mod.rs");
    assert!(
        !event_mod.contains("<ContentContainer")
            && !event_mod.contains("use orbital::components::{ContentContainer")
            && !event_mod.contains("Size240"),
        "event explore canvas must not use ContentContainer / Size240 gutter"
    );
}

#[test]
fn explore_toolbar_refresh_and_view_select_happy_path() {
    let metric_toolbar = read_app("pages/metric_explore/components/metric_toolbar.rs");
    assert!(
        metric_toolbar.contains("Refresh Data") && metric_toolbar.contains("spectra-refresh-data"),
        "metric toolbar must expose Refresh Data next to the time range"
    );
    assert!(
        metric_toolbar.contains("TimeRangePicker"),
        "metric toolbar must keep TimeRangePicker sibling for Refresh"
    );

    let event_toolbar = read_app("pages/event_explore/components/event_toolbar.rs");
    assert!(
        event_toolbar.contains("Refresh Data") && event_toolbar.contains("spectra-refresh-data"),
        "event toolbar must expose Refresh Data"
    );

    let view_picker = read_app("components/query/event_view_picker.rs");
    assert!(
        view_picker.contains("Select") && view_picker.contains("EventExploreView"),
        "EventViewPicker must be an Orbital Select bound to EventExploreView"
    );

    let time_range = read_app("components/query/time_range_picker.rs");
    assert!(
        !time_range.contains("let _ = selected_secs"),
        "TimeRangePicker must use selected_secs for Primary appearance"
    );
    assert!(
        time_range.contains("ButtonAppearance::Primary"),
        "TimeRangePicker must mark the selected preset Primary"
    );
}

#[test]
fn schema_detail_fields_table_happy_path() {
    let detail = read_app("pages/schema_detail/components/schema_detail_body.rs");
    assert!(
        detail.contains("SchemaFieldDto")
            || detail.contains("schema_fields_to_rows")
            || detail.contains("fields"),
        "schema detail must render SchemaDetailDto.fields"
    );
    assert!(
        detail.contains("spectra-schema-fields-table"),
        "schema fields table needs a stable testid"
    );
    assert!(
        detail.contains("CardContent") && detail.contains("EmptyState"),
        "fields card must use CardContent and EmptyState for empty fields"
    );
}

#[test]
fn index_drop_schema_testid_sad_path() {
    let index = read_app("pages/schema_index/mod.rs");
    assert!(
        index.contains("data_testid=\"schema-index-page\""),
        "dropping schema-index-page breaks host / future Playwright parity"
    );
    let event = read_app("pages/event_explore/mod.rs");
    assert!(
        event.contains("data_testid=\"spectra-event-explore-panel\"")
            || event.contains("data-testid=\"spectra-event-explore-panel\""),
        "dropping spectra-event-explore-panel breaks host / future Playwright parity"
    );
    let metric = read_app("pages/metric_explore/mod.rs");
    assert!(
        metric.contains("data_testid=\"spectra-metric-explore-panel\"")
            || metric.contains("data-testid=\"spectra-metric-explore-panel\""),
        "dropping spectra-metric-explore-panel breaks host / future Playwright parity"
    );
}

#[test]
fn detail_pages_bindings_happy_path() {
    let detail = read_app("pages/schema_detail/components/schema_detail_body.rs");
    assert!(
        detail.contains("get_schema_metadata"),
        "SchemaDetailBody must bind get_schema_metadata"
    );
    let home = read_app("pages/home.rs");
    assert!(
        home.contains("/spectra/schema"),
        "SpectraHomePage must redirect into the schema index funnel"
    );
}

#[test]
fn detail_pages_missing_bindings_sad_path() {
    let detail = read_app("pages/schema_detail/components/schema_detail_body.rs");
    assert!(
        detail.contains("get_schema_metadata"),
        "schema detail must bind get_schema_metadata"
    );
    assert!(
        !detail.contains("unimplemented!"),
        "schema detail must not ship unimplemented placeholders"
    );
}

#[test]
fn permission_manifest_query_table_happy_path() {
    let perms = read_app("permissions.rs");
    for needle in [
        "domain_key = \"spectra\"",
        "QueryTable",
        "UfPermissionManifest",
    ] {
        assert!(
            perms.contains(needle),
            "SpectraPermission manifest missing `{needle}`"
        );
    }
}

#[test]
fn protected_spectra_host_matches_uf_app_happy_path() {
    let host =
        fs::read_to_string(workspace_root().join("examples/protected-spectra-host/src/main.rs"))
            .expect("protected-spectra-host main.rs");
    for needle in [
        "\"app_id\": \"spectra\"",
        "\"route_path\": \"/spectra\"",
        "\"auth_gate\": \"RequireAuthenticated\"",
        "\"admin_permission\": \"QueryTable\"",
        "schema_metadata_list",
    ] {
        assert!(
            host.contains(needle),
            "protected-spectra-host missing contract `{needle}`"
        );
    }
    let lib = read_app("lib.rs");
    assert!(
        lib.contains("id: \"spectra\"") && lib.contains("route_path: \"/spectra\""),
        "host inventory must stay aligned with uf_app!"
    );
    let layout = read_app("layout/spectra_layout.rs");
    assert!(
        layout.contains("RequireAuthenticated"),
        "host auth_gate must stay aligned with SpectraLayout guard"
    );
    let perms = read_app("permissions.rs");
    assert!(
        perms.contains("QueryTable"),
        "host admin_permission must stay aligned with SpectraPermission"
    );
}

#[test]
fn lazy_routes_wire_pages_happy_path() {
    let lazy = read_app("lazy_routes.rs");
    for needle in [
        "SpectraHomePage",
        "SchemaIndexPage",
        "SchemaDetailPage",
        "EventExplorePage",
        "MetricExplorePage",
        "SpectraLayout",
    ] {
        assert!(
            lazy.contains(needle),
            "lazy_routes missing page wire `{needle}`"
        );
    }
}

#[test]
fn ops_path_helpers_wire_detail_hrefs_happy_path() {
    let schema_card = read_app("components/schema/schema_card.rs");
    let quick = read_app("pages/schema_detail/components/quick_actions_card.rs");
    for (label, src) in [
        ("schema_card", schema_card.as_str()),
        ("quick_actions_card", quick.as_str()),
    ] {
        assert!(
            src.contains("spectra_schema_explore_path")
                || src.contains("spectra_metric_explore_path")
                || src.contains("spectra_schema_path"),
            "{label} must build detail/explore hrefs via spectra_backend path helpers"
        );
        assert!(
            !src.contains("format!(\"/spectra/schema/{name}")
                && !src.contains("format!(\"/spectra/metric/{name}"),
            "{label} must not interpolate raw names into /spectra hrefs"
        );
    }
}

#[test]
fn ops_path_helpers_drop_encoding_sad_path() {
    let schema_card = read_app("components/schema/schema_card.rs");
    assert!(
        schema_card.contains("spectra_schema_path")
            && schema_card.contains("spectra_schema_explore_path")
            && schema_card.contains("spectra_metric_explore_path"),
        "dropping spectra_*_path helpers reopens path-segment smuggling via schema names"
    );
}

#[test]
fn get_schema_metadata_validates_name_happy_path() {
    let server = read_app("server/mod.rs");
    let start = server
        .find("pub async fn get_schema_metadata")
        .expect("get_schema_metadata");
    let body = &server[start..start + 450.min(server.len() - start)];
    assert!(
        body.contains("validate_spectra_query_name(&name)"),
        "get_schema_metadata must reject blank/unsafe/oversized names before catalog lookup"
    );
}

#[test]
fn get_schema_metadata_drop_validate_sad_path() {
    let server = read_app("server/mod.rs");
    let start = server
        .find("pub async fn get_schema_metadata")
        .expect("get_schema_metadata");
    let body = &server[start..start + 450.min(server.len() - start)];
    assert!(
        body.contains("validate_spectra_query_name(&name)"),
        "dropping validate_spectra_query_name on get_schema_metadata lets unsafe ids reach Spectra IO"
    );
}
