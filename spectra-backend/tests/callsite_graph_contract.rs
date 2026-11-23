//! Callsite graph contracts against mem Spectra backends.

#![allow(missing_docs, clippy::expect_used)]

use std::sync::Arc;

use chrono::Utc;
use serde_json::json;
use spectra::helpers::PlatformSmokeEventLogger;
use spectra::{MemEventsBackend, MemMetricsBackend, Spectra};
use spectra_backend::{execute_callsite_graph, CallsiteScope, EmptyReason, SiteKind};
use spectra_core::{EventStorageBackend, MetricsStorageBackend, SpectraRouter};

/// Generated loggers emit through a process-wide sink bound by the latest build, so tests that
/// build a Spectra run one at a time.
static SPECTRA_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const SMOKE_TABLE: &str = "platform_smoke_event";
const RAW_TABLE: &str = "integ.raw_events";
const METRIC: &str = "integ.cache_hits";

fn mem_spectra(events: Arc<MemEventsBackend>) -> Spectra {
    mem_spectra_with(events, Arc::new(MemMetricsBackend::new()))
}

fn mem_spectra_with(events: Arc<MemEventsBackend>, metrics: Arc<MemMetricsBackend>) -> Spectra {
    let metrics: Arc<dyn MetricsStorageBackend> = metrics;
    let events: Arc<dyn EventStorageBackend> = events;
    Spectra::builder()
        .metrics_backend(metrics)
        .events_backend(events)
        .embedded()
        .build()
        .expect("mem spectra")
}

fn names(items: &[&str]) -> Vec<String> {
    items.iter().map(|t| (*t).to_owned()).collect()
}

async fn graph_in(
    router: Option<&SpectraRouter>,
    scope: &CallsiteScope,
) -> spectra_backend::CallsiteGraph {
    execute_callsite_graph(
        router,
        scope,
        Utc::now() - chrono::Duration::minutes(5),
        Utc::now() + chrono::Duration::minutes(1),
    )
    .await
}

async fn graph_for(router: &SpectraRouter, tables: &[&str]) -> spectra_backend::CallsiteGraph {
    let scope = CallsiteScope {
        tables: names(tables),
        ..CallsiteScope::default()
    };
    graph_in(Some(router), &scope).await
}

#[tokio::test]
async fn generated_logger_rows_become_emit_sites_happy_path() {
    let _guard = SPECTRA_LOCK.lock().await;
    let events = Arc::new(MemEventsBackend::new());
    let spectra = mem_spectra(Arc::clone(&events));

    let first_line = line!() + 1;
    PlatformSmokeEventLogger::log("first".to_string());
    let second_line = line!() + 1;
    PlatformSmokeEventLogger::log("second".to_string());
    PlatformSmokeEventLogger::log("second again".to_string());
    // The logger hands rows to an async sink, so wait until all three have landed.
    let router = spectra.router();
    let mut graph = graph_for(&router, &[SMOKE_TABLE]).await;
    for _ in 0..100 {
        if graph.scanned >= 3 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        spectra.flush_persist().await.expect("flush");
        graph = graph_for(&router, &[SMOKE_TABLE]).await;
    }

    assert_eq!(graph.tables, vec![SMOKE_TABLE.to_owned()]);
    assert_eq!(graph.untracked, 0);
    assert!(!graph.truncated);
    let lines: Vec<u32> = graph.sites.iter().map(|s| s.line).collect();
    assert!(lines.contains(&first_line), "sites: {:?}", graph.sites);
    assert!(lines.contains(&second_line), "sites: {:?}", graph.sites);
    assert!(graph
        .sites
        .iter()
        .all(|s| s.file.ends_with("callsite_graph_contract.rs")
            && s.kind == SiteKind::Table
            && s.target == SMOKE_TABLE));
    let total: u64 = graph.sites.iter().map(|s| s.count).sum();
    assert_eq!(total, 3);
}

#[tokio::test]
async fn rows_without_callsite_are_counted_not_graphed_sad_path() {
    let _guard = SPECTRA_LOCK.lock().await;
    let events = Arc::new(MemEventsBackend::new());
    events
        .append_row(RAW_TABLE, &json!({ "message": "raw" }), Utc::now(), None)
        .await
        .expect("append");
    let spectra = mem_spectra(events);

    let graph = graph_for(&spectra.router(), &[RAW_TABLE]).await;

    assert_eq!(graph.sites, Vec::new());
    assert_eq!(graph.tables, Vec::<String>::new());
    assert_eq!(graph.scanned, 1);
    assert_eq!(graph.untracked, 1);
    assert_eq!(graph.empty, Some(EmptyReason::NothingLogged));
}

#[tokio::test]
async fn metric_points_become_emit_sites_happy_path() {
    let _guard = SPECTRA_LOCK.lock().await;
    let metrics = Arc::new(MemMetricsBackend::new());
    let callsite = json!({
        "file": "fernwood-api/src/gateway/latency.rs",
        "line": 61,
        "module": "fernwood_api::gateway::latency",
    })
    .to_string();
    let now = Utc::now();
    for offset in 0..3 {
        metrics
            .record_counter(
                METRIC,
                &json!({ "region": "us", "_callsite": callsite }),
                1,
                now - chrono::Duration::seconds(offset),
            )
            .await
            .expect("record");
    }
    // A point without a callsite (a gauge or direct try_record_*) is not a site.
    metrics
        .record_counter(METRIC, &json!({ "region": "us" }), 1, now)
        .await
        .expect("record");
    let spectra = mem_spectra_with(Arc::new(MemEventsBackend::new()), metrics);

    let scope = CallsiteScope {
        metrics: names(&[METRIC]),
        ..CallsiteScope::default()
    };
    let graph = graph_in(Some(&spectra.router()), &scope).await;

    assert_eq!(graph.empty, None);
    assert_eq!(graph.metrics, vec![METRIC.to_owned()]);
    assert_eq!(graph.tables, Vec::<String>::new());
    assert_eq!(graph.untracked, 0);
    let [site] = graph.sites.as_slice() else {
        panic!("one metric site expected, got {:?}", graph.sites);
    };
    assert_eq!(site.kind, SiteKind::Metric);
    assert_eq!(site.target, METRIC);
    assert_eq!(site.file, "fernwood-api/src/gateway/latency.rs");
    assert_eq!(site.line, 61);
    assert_eq!(site.count, 3);
    assert_eq!(
        site.module.as_deref(),
        Some("fernwood_api::gateway::latency")
    );
    assert_eq!(site.module_in_crate(), Some("gateway::latency"));
}

#[tokio::test]
async fn no_router_reports_no_backend_sad_path() {
    let scope = CallsiteScope {
        tables: names(&[SMOKE_TABLE]),
        metrics: names(&[METRIC]),
        hidden_tables: names(&[RAW_TABLE]),
    };
    let graph = graph_in(None, &scope).await;
    assert_eq!(graph.empty, Some(EmptyReason::NoBackend));
    assert_eq!(graph.sites, Vec::new());
    assert_eq!(graph.hidden_tables, names(&[RAW_TABLE]));
}

#[tokio::test]
async fn nothing_readable_reports_no_grants_sad_path() {
    let _guard = SPECTRA_LOCK.lock().await;
    let spectra = mem_spectra(Arc::new(MemEventsBackend::new()));
    let scope = CallsiteScope {
        hidden_tables: names(&[SMOKE_TABLE]),
        ..CallsiteScope::default()
    };
    let graph = graph_in(Some(&spectra.router()), &scope).await;
    assert_eq!(graph.empty, Some(EmptyReason::NoGrants));
    assert_eq!(graph.scanned, 0);
    assert_eq!(graph.hidden_tables, names(&[SMOKE_TABLE]));
}

#[tokio::test]
async fn empty_window_reports_nothing_logged_sad_path() {
    let _guard = SPECTRA_LOCK.lock().await;
    let spectra = mem_spectra(Arc::new(MemEventsBackend::new()));
    let scope = CallsiteScope {
        tables: names(&[RAW_TABLE]),
        metrics: names(&[METRIC]),
        ..CallsiteScope::default()
    };
    let graph = graph_in(Some(&spectra.router()), &scope).await;
    assert_eq!(graph.empty, Some(EmptyReason::NothingLogged));
    assert_eq!(graph.sites, Vec::new());
}

#[tokio::test]
async fn hidden_table_is_reported_and_not_scanned_happy_path() {
    let _guard = SPECTRA_LOCK.lock().await;
    let events = Arc::new(MemEventsBackend::new());
    let callsite =
        json!({ "file": "shop/src/checkout.rs", "line": 12, "module": "shop::checkout" });
    for table in [RAW_TABLE, SMOKE_TABLE] {
        events
            .append_row(
                table,
                &json!({ "message": "row", "_callsite": callsite }),
                Utc::now(),
                None,
            )
            .await
            .expect("append");
    }
    let spectra = mem_spectra(events);

    let scope = CallsiteScope {
        tables: names(&[RAW_TABLE]),
        hidden_tables: names(&[SMOKE_TABLE]),
        ..CallsiteScope::default()
    };
    let graph = graph_in(Some(&spectra.router()), &scope).await;

    assert_eq!(graph.empty, None);
    assert_eq!(graph.tables, names(&[RAW_TABLE]));
    assert_eq!(graph.hidden_tables, names(&[SMOKE_TABLE]));
    assert_eq!(graph.scanned, 1, "the hidden table is never read");
    assert!(graph.sites.iter().all(|s| s.target == RAW_TABLE));
    assert_eq!(graph.sites[0].module_in_crate(), Some("checkout"));
}
