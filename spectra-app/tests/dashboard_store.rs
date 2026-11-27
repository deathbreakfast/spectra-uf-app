//! Saved-dashboard store against in-memory Valence and real Gauge permission bundles.
//!
//! Every scenario acts through `as_user` on one shared database, so privacy is checked as
//! the person acting. System is used only to seed fixture users and to play the account
//! deletion job, which starts as System.

#![allow(missing_docs, clippy::expect_used, clippy::unwrap_used)]

use std::sync::Arc;

use chrono::Utc;
use gauge::resource_permissions::delete_resource_permission_bundle;
use spectra_backend::dashboards::policies::SPECTRA_DASHBOARD;
use spectra_backend::dashboards::{sharing, store};
use spectra_backend::dashboards::{
    ChartKind, DashboardError, DashboardScope, Layout, Panel, PanelSource, Principal, Section,
    TableOpts,
};
use spectra_backend::generated::{
    SpectraDashboard, SpectraDashboardFavourite, SpectraDashboardVersion,
};
use tokio::sync::Mutex;
use valence::deletion::dag::{DeletionAction, DeletionDag};
use valence::{
    register_backend_logical_names, router_key, Actor, DatabaseBackend, DatabaseRouter,
    InMemoryBackend, Model, RegisterBackendLogicalNamesOptions, Valence, MEM_ENGINE_ID,
    SQLITE_ENGINE_ID,
};

const ALICE: &str = "alice";
const BOB: &str = "bob";
const CAROL: &str = "carol";
const BUILTIN: &str = "photon-throughput";

/// Valence's schema registry and caches are process-wide; run one scenario at a time.
static SERIAL: Mutex<()> = Mutex::const_new(());

async fn seed_user(id: &str, system: &Valence) {
    use lepton_identity::generated::{User, UserStatus, UserUserType};
    let now = Utc::now();
    let user = User::new(
        Some(UserUserType::Person),
        Some("test-password-hash".into()),
        Some(UserStatus::Active),
        None,
        None,
        Some(now),
        None,
        None,
        now,
        now,
    )
    .expect("build user");
    User::upsert(id, user, system, valence::use_!(r"**Test:** Fixture **User** save for the dashboard store tests, so each scenario has signed-in people to act as. CI and developers running the suite only."))
        .await
        .expect("seed user");
}

/// Fresh in-memory database with Alice, Bob and Carol, as the System fixture actor.
async fn setup() -> Valence {
    valence::deletion::register_noop_deletion_dispatcher_for_tests();
    valence::clear_for_test();
    if std::env::var_os("VALENCE_OWNERSHIP_UNIFIED_FETCH").is_none() {
        // SAFETY: test harness only, before the first ownership read.
        unsafe { std::env::set_var("VALENCE_OWNERSHIP_UNIFIED_FETCH", "0") };
    }
    let backend: Arc<dyn DatabaseBackend> = Arc::new(InMemoryBackend::new());
    let mut router = DatabaseRouter::new();
    register_backend_logical_names(
        &mut router,
        Arc::clone(&backend),
        gauge::embedded_surreal::EMBEDDED_SURREAL_LOGICAL_NAMES,
        RegisterBackendLogicalNamesOptions {
            register_alias_engine_id: Some(SQLITE_ENGINE_ID),
        },
    );
    router.register(
        router_key(gauge::embedded_surreal::LOGICAL_NAME, SQLITE_ENGINE_ID),
        Arc::clone(&backend),
    );
    let system = Valence::builder()
        .database_router(Arc::new(router))
        .default_backend_key(router_key(
            gauge::embedded_surreal::LOGICAL_NAME,
            MEM_ENGINE_ID,
        ))
        .with_actor(Actor::System {
            operation: "spectra_dashboard_store_test".into(),
        })
        .build()
        .expect("build valence");
    for user in [ALICE, BOB, CAROL] {
        seed_user(user, &system).await;
    }
    system
}

fn as_user(base: &Valence, user_id: &str) -> Valence {
    base.with_actor(Actor::User {
        user_id: user_id.into(),
    })
}

fn table_panel(id: &str) -> Panel {
    Panel {
        id: id.into(),
        title: "Recent checkouts".into(),
        source: PanelSource::EventTable {
            table: "checkout.events".into(),
        },
        chart: ChartKind::Table,
        measure: None,
        bucket: None,
        group_by: Vec::new(),
        filters: Vec::new(),
        table: Some(TableOpts {
            columns: vec!["ts".into(), "status".into()],
            sort: None,
            limit: 100,
        }),
        layout: Layout {
            col: 0,
            span: 12,
            row: 0,
            height: 4,
        },
        keep_window: None,
    }
}

#[tokio::test]
async fn create_save_then_stale_save_conflicts() {
    let _serial = SERIAL.lock().await;
    let system = setup().await;
    let alice = as_user(&system, ALICE);

    let created = store::create(&alice, "Checkout health", "Errors and latency", None)
        .await
        .expect("create");
    assert_eq!(created.version, 1);
    assert_eq!(created.owner.as_deref(), Some(ALICE));
    assert!(!created.read_only);

    let mut edited = created.clone();
    edited.sections.push(Section {
        id: "s1".into(),
        title: "Checkout".into(),
        collapsed: false,
        panels: vec![table_panel("p1")],
    });
    let saved = store::save(&alice, &edited, 1, Some("Add a table panel"), None)
        .await
        .expect("save at version 1");
    assert_eq!(saved.version, 2);
    let reloaded = store::get(&alice, &created.id, None).await.expect("get");
    assert_eq!(reloaded.sections, edited.sections);

    let stale = store::save(&alice, &edited, 1, None, None).await;
    assert_eq!(stale, Err(DashboardError::Conflict { current: 2 }));

    let first = store::get(&alice, &created.id, Some(1)).await.expect("v1");
    assert_eq!(first.sections, Vec::new());
}

#[tokio::test]
async fn viewer_reads_but_cannot_save_and_outsider_cannot_read() {
    let _serial = SERIAL.lock().await;
    let system = setup().await;
    let alice = as_user(&system, ALICE);
    let bob = as_user(&system, BOB);
    let carol = as_user(&system, CAROL);

    let d = store::create(&alice, "Shared view", "", None)
        .await
        .expect("create");
    let access = sharing::share(&alice, &d.id, &[], &[Principal::User(BOB.into())])
        .await
        .expect("share");
    assert_eq!(access.viewers, vec![Principal::User(BOB.into())]);
    assert_eq!(access.editors, Vec::new());

    let seen = store::get(&bob, &d.id, None)
        .await
        .expect("viewer can read");
    assert_eq!(seen.title, "Shared view");
    let shared = store::list(&bob, DashboardScope::Shared, None)
        .await
        .expect("list");
    assert!(shared.iter().any(|s| s.id == d.id));
    let bob_mine = store::list(&bob, DashboardScope::Mine, None)
        .await
        .expect("mine");
    assert_eq!(bob_mine, Vec::new());

    let bob_access = sharing::access(&bob, &d.id).await.expect("access");
    assert!(!bob_access.can_edit && !bob_access.can_delete && !bob_access.can_share);
    assert_eq!(
        bob_access.viewers,
        vec![Principal::User(BOB.into())],
        "a viewer sees the lists the owner saved"
    );
    assert_eq!(bob_access.editors, Vec::new());

    let mut edit = seen.clone();
    edit.title = "Renamed by a viewer".into();
    assert_eq!(
        store::save(&bob, &edit, 1, None, None).await,
        Err(DashboardError::Forbidden)
    );
    assert_eq!(
        store::get(&alice, &d.id, None).await.expect("get").version,
        1
    );

    assert_eq!(
        store::get(&carol, &d.id, None).await,
        Err(DashboardError::NotFound)
    );
    assert_eq!(
        sharing::share(&bob, &d.id, &[Principal::User(BOB.into())], &[]).await,
        Err(DashboardError::Forbidden)
    );
}

#[tokio::test]
async fn editor_saves_but_only_the_owner_deletes() {
    let _serial = SERIAL.lock().await;
    let system = setup().await;
    let alice = as_user(&system, ALICE);
    let bob = as_user(&system, BOB);

    let d = store::create(&alice, "Team board", "", None)
        .await
        .expect("create");
    sharing::share(&alice, &d.id, &[Principal::User(BOB.into())], &[])
        .await
        .expect("share");

    let mut edit = store::get(&bob, &d.id, None).await.expect("editor reads");
    edit.description = "Edited by Bob".into();
    let saved = store::save(&bob, &edit, 1, None, Some("MCP".into()))
        .await
        .expect("editor saves");
    assert_eq!(saved.version, 2);
    let versions = store::list_versions(&alice, &d.id).await.expect("versions");
    assert_eq!(versions[0].author.as_deref(), Some(BOB));
    assert_eq!(versions[0].via.as_deref(), Some("MCP"));

    store::set_favourite(&bob, &d.id, true)
        .await
        .expect("bob stars it");
    assert_eq!(
        store::delete(&bob, &d.id).await,
        Err(DashboardError::Forbidden)
    );
    assert!(store::get(&alice, &d.id, None).await.is_ok());

    store::delete(&alice, &d.id).await.expect("owner deletes");
    assert_eq!(
        store::get(&alice, &d.id, None).await,
        Err(DashboardError::NotFound)
    );
    let left_versions = SpectraDashboardVersion::query(&system, valence::use_!(r"**Test:** Check that deleting a **Spectra dashboard** also removed its saved versions. CI and developers running the suite only.")).await.expect("versions");
    assert!(left_versions.is_empty());
    let left_favourites = SpectraDashboardFavourite::query(&system, valence::use_!(r"**Test:** Check that deleting a **Spectra dashboard** also removed other people's stars on it. CI and developers running the suite only.")).await.expect("favourites");
    assert!(left_favourites.is_empty());
}

#[tokio::test]
async fn restore_saves_an_old_version_as_the_newest() {
    let _serial = SERIAL.lock().await;
    let system = setup().await;
    let alice = as_user(&system, ALICE);

    let d = store::create(&alice, "Original title", "", None)
        .await
        .expect("create");
    let mut edit = d.clone();
    edit.title = "Changed title".into();
    store::save(&alice, &edit, 1, None, None)
        .await
        .expect("save");

    let restored = store::restore(&alice, &d.id, 1, None)
        .await
        .expect("restore");
    assert_eq!(restored.version, 3);
    assert_eq!(restored.title, "Original title");

    let versions = store::list_versions(&alice, &d.id).await.expect("versions");
    let numbers: Vec<u32> = versions.iter().map(|v| v.number).collect();
    assert_eq!(numbers, vec![3, 2, 1]);
    assert_eq!(versions[0].summary, "Restored version 1");
    assert_eq!(
        store::get(&alice, &d.id, Some(2)).await.expect("v2").title,
        "Changed title"
    );
    assert_eq!(
        store::restore(&alice, &d.id, 9, None).await,
        Err(DashboardError::NotFound)
    );
}

#[tokio::test]
async fn builtins_are_listed_and_read_only() {
    let _serial = SERIAL.lock().await;
    let system = setup().await;
    let alice = as_user(&system, ALICE);

    let all = store::list(&alice, DashboardScope::All, None)
        .await
        .expect("all");
    let builtin = all
        .iter()
        .find(|s| s.id == BUILTIN)
        .expect("built-in listed");
    assert!(builtin.read_only);
    assert_eq!(builtin.owner, None);
    assert!(store::list(&alice, DashboardScope::Mine, None)
        .await
        .expect("mine")
        .iter()
        .all(|s| !s.read_only));

    let dashboard = store::get(&alice, BUILTIN, None)
        .await
        .expect("get built-in");
    assert!(dashboard.read_only);
    assert_eq!(
        store::save(&alice, &dashboard, 0, None, None).await,
        Err(DashboardError::ReadOnly)
    );
    assert_eq!(
        store::delete(&alice, BUILTIN).await,
        Err(DashboardError::ReadOnly)
    );
    assert_eq!(
        sharing::share(&alice, BUILTIN, &[], &[]).await,
        Err(DashboardError::ReadOnly)
    );
    assert_eq!(
        store::restore(&alice, BUILTIN, 0, None).await,
        Err(DashboardError::ReadOnly)
    );

    let copy = store::create(&alice, "My Photon board", "", Some(BUILTIN))
        .await
        .expect("copy a built-in");
    assert_eq!(copy.sections, dashboard.sections);
    assert_eq!(copy.owner.as_deref(), Some(ALICE));
}

#[tokio::test]
async fn favourites_and_search_shape_the_list() {
    let _serial = SERIAL.lock().await;
    let system = setup().await;
    let alice = as_user(&system, ALICE);

    let a = store::create(&alice, "Alpha board", "", None)
        .await
        .expect("a");
    let z = store::create(&alice, "Zulu board", "payments", None)
        .await
        .expect("z");
    store::set_favourite(&alice, &z.id, true)
        .await
        .expect("star");
    store::set_favourite(&alice, &z.id, true)
        .await
        .expect("star twice");

    let mine = store::list(&alice, DashboardScope::Mine, None)
        .await
        .expect("mine");
    let ids: Vec<&str> = mine.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec![z.id.as_str(), a.id.as_str()]);
    assert!(mine[0].favourite && !mine[1].favourite);

    let found = store::list(&alice, DashboardScope::All, Some("PAYMENTS"))
        .await
        .expect("search");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, z.id);

    store::set_favourite(&alice, &z.id, false)
        .await
        .expect("unstar");
    let mine = store::list(&alice, DashboardScope::Mine, None)
        .await
        .expect("mine");
    assert!(mine.iter().all(|s| !s.favourite));
    assert_eq!(
        store::set_favourite(&alice, "missing", true).await,
        Err(DashboardError::NotFound)
    );
}

#[tokio::test]
async fn invalid_dashboards_are_refused() {
    let _serial = SERIAL.lock().await;
    let system = setup().await;
    let alice = as_user(&system, ALICE);

    assert!(matches!(
        store::create(&alice, "   ", "", None).await,
        Err(DashboardError::Validation { .. })
    ));
    let d = store::create(&alice, "Board", "", None)
        .await
        .expect("create");
    let mut bad = d.clone();
    let mut panel = table_panel("p1");
    panel.layout.span = 0;
    bad.sections.push(Section {
        id: "s1".into(),
        title: String::new(),
        collapsed: false,
        panels: vec![panel],
    });
    assert!(matches!(
        store::save(&alice, &bad, 1, None, None).await,
        Err(DashboardError::Validation { .. })
    ));
}

#[tokio::test]
async fn user_deletion_clears_their_favourites_and_owned_dashboards() {
    let _serial = SERIAL.lock().await;
    let system = setup().await;
    let alice = as_user(&system, ALICE);
    let carol = as_user(&system, CAROL);

    let carols = store::create(&carol, "Carol's board", "", None)
        .await
        .expect("carol's");
    let alices = store::create(&alice, "Alice's board", "", None)
        .await
        .expect("alice's");
    sharing::share(&alice, &alices.id, &[Principal::User(CAROL.into())], &[])
        .await
        .expect("share with carol");
    let mut edit = store::get(&carol, &alices.id, None)
        .await
        .expect("carol reads");
    edit.description = "Carol was here".into();
    store::save(&carol, &edit, 1, None, None)
        .await
        .expect("carol saves");
    store::set_favourite(&carol, &alices.id, true)
        .await
        .expect("carol stars");
    store::set_favourite(&alice, &alices.id, true)
        .await
        .expect("alice stars");

    // The account deletion job starts as System. Gauge's rows that point at the user
    // (the bundle permissions Carol created, her principal) are Restrict, so that job clears
    // them first; this test plays that step, then walks the rest of the user's graph.
    let blocked = DeletionDag::compute("user", CAROL, &system)
        .await
        .expect("dag");
    assert!(blocked
        .restrict_violations
        .iter()
        .all(|r| r.blocking_table.starts_with("permission")));
    delete_resource_permission_bundle(&system, SPECTRA_DASHBOARD, &carols.id)
        .await
        .expect("drop carol's dashboard bundle");
    let principals = gauge::generated::PermissionUserPrincipal::query(&system, valence::use_!(r"**Test:** Find the Gauge **principal** of the user being deleted, so the test can play Gauge's part of account deletion. CI and developers running the suite only."))
        .where_user(valence::RecordPredicate::Equals(valence::RecordId::new("user", CAROL)))
        .await
        .expect("principals");
    for principal in principals {
        let id = valence::extract_id_from_record(principal.id().expect("id")).expect("bare id");
        valence::delete_entity_now("permission_user_principal", &id, &system)
            .await
            .expect("drop principal");
    }

    let dag = DeletionDag::compute("user", CAROL, &system)
        .await
        .expect("dag");
    assert!(
        dag.restrict_violations.is_empty(),
        "{:?}",
        dag.restrict_violations
            .iter()
            .map(|r| (&r.blocking_table, &r.blocking_field))
            .collect::<Vec<_>>()
    );
    let ours: Vec<_> = dag
        .nodes
        .iter()
        .filter(|n| n.table.starts_with("spectra_dashboard"))
        .collect();
    assert!(ours.iter().any(|n| n.table == "spectra_dashboard"
        && n.action == DeletionAction::CascadeDelete
        && n.record_id.contains(&carols.id)));
    for node in &ours {
        valence::deletion::apply_deletion_node(node, &system)
            .await
            .expect("apply deletion node");
    }

    let left = SpectraDashboard::query(&system, valence::use_!(r"**Test:** Check which **Spectra dashboards** remain after a user is deleted. CI and developers running the suite only.")).await.expect("dashboards");
    let left_ids: Vec<String> = left
        .iter()
        .filter_map(|d| d.id().map(|r| valence::extract_id_from_record(r).unwrap()))
        .collect();
    assert_eq!(left_ids, vec![alices.id.clone()]);

    let favourites = SpectraDashboardFavourite::query(&system, valence::use_!(r"**Test:** Check which **dashboard stars** remain after a user is deleted. CI and developers running the suite only.")).await.expect("favourites");
    assert_eq!(favourites.len(), 1, "only Alice's star is left");

    let versions = store::list_versions(&alice, &alices.id)
        .await
        .expect("versions");
    let carols_save = versions.iter().find(|v| v.number == 2).expect("v2");
    assert_eq!(
        carols_save.author, None,
        "the deleted user's name is cleared"
    );
}
