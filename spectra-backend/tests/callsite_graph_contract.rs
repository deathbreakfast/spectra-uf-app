//! Callsite graph contracts against mem Spectra backends.

#![allow(missing_docs, clippy::expect_used)]

use std::sync::Arc;

use chrono::Utc;
use serde_json::json;
use spectra::helpers::PlatformSmokeEventLogger;
use spectra::{MemEventsBackend, MemMetricsBackend, Spectra};
use spectra_backend::execute_callsite_graph;
use spectra_core::{EventStorageBackend, MetricsStorageBackend, SpectraRouter};

const SMOKE_TABLE: &str = "platform_smoke_event";
const RAW_TABLE: &str = "integ.raw_events";

fn mem_spectra(events: Arc<MemEventsBackend>) -> Spectra {
    let metrics: Arc<dyn MetricsStorageBackend> = Arc::new(MemMetricsBackend::new());
    let events: Arc<dyn EventStorageBackend> = events;
    Spectra::builder()
        .metrics_backend(metrics)
        .events_backend(events)
        .embedded()
        .build()
        .expect("mem spectra")
}

async fn graph_for(router: &SpectraRouter, tables: &[&str]) -> spectra_backend::CallsiteGraph {
    let tables: Vec<String> = tables.iter().map(|t| (*t).to_owned()).collect();
    execute_callsite_graph(
        router,
        &tables,
        Utc::now() - chrono::Duration::minutes(5),
        Utc::now() + chrono::Duration::minutes(1),
    )
    .await
}

#[tokio::test]
async fn generated_logger_rows_become_emit_sites_happy_path() {
    let events = Arc::new(MemEventsBackend::new());
    let spectra = mem_spectra(Arc::clone(&events));

    let first_line = line!() + 1;
    PlatformSmokeEventLogger::log("first".to_string());
    let second_line = line!() + 1;
    PlatformSmokeEventLogger::log("second".to_string());
    PlatformSmokeEventLogger::log("second again".to_string());
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    spectra.flush_persist().await.expect("flush");

    let graph = graph_for(&spectra.router(), &[SMOKE_TABLE]).await;

    assert_eq!(graph.tables, vec![SMOKE_TABLE.to_owned()]);
    assert_eq!(graph.untracked, 0);
    assert!(!graph.truncated);
    let lines: Vec<u32> = graph.sites.iter().map(|s| s.line).collect();
    assert!(lines.contains(&first_line), "sites: {:?}", graph.sites);
    assert!(lines.contains(&second_line), "sites: {:?}", graph.sites);
    assert!(graph
        .sites
        .iter()
        .all(|s| s.file.ends_with("callsite_graph_contract.rs") && s.table == SMOKE_TABLE));
    let total: u64 = graph.sites.iter().map(|s| s.count).sum();
    assert_eq!(total, 3);
}

#[tokio::test]
async fn rows_without_callsite_are_counted_not_graphed_sad_path() {
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
}

#[tokio::test]
async fn empty_table_list_yields_empty_graph_sad_path() {
    let spectra = mem_spectra(Arc::new(MemEventsBackend::new()));
    let graph = graph_for(&spectra.router(), &[]).await;
    assert_eq!(graph, spectra_backend::CallsiteGraph::default());
}
