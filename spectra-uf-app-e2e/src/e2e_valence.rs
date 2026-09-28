//! Process-wide Valence + Higgs + Gauge for Playwright.
#![allow(dead_code)]

use std::sync::{Arc, Mutex, OnceLock};

use chrono::Utc;
use gauge::manifest_sync::{
    sync_permission_manifests, PermissionDomainInput, PermissionInput, PermissionManifestInput,
};
use gauge::service;
use gauge::super_user::SUPER_USER_GROUP_NAME;
use higgs::actor_policy::external_actor_json_policy;
use higgs::{HiggsConfig, HiggsValenceFactory};
use valence::{
    register_backend_logical_names, router_key, Actor, DatabaseBackend, DatabaseRouter,
    InMemoryBackend, Model, RegisterBackendLogicalNamesOptions, RouterValenceFactory,
    RouterValenceFactoryConfig, Valence, ValenceFactory, MEM_ENGINE_ID, SQLITE_ENGINE_ID,
};

struct E2eState {
    router: Arc<DatabaseRouter>,
    higgs: Arc<HiggsConfig>,
    default_backend_key: String,
    fixtures: Mutex<FixtureIds>,
}

/// Stable fixture ids exposed to seed JSON / Playwright.
#[derive(Clone, Debug, Default)]
pub struct FixtureIds {
    pub event_table: String,
    pub empty_event_table: String,
    pub metric_name: String,
    pub empty_metric_name: String,
    /// Recent events written for admin seed (excludes stale row).
    pub seeded_event_count: u32,
}

static E2E_STATE: OnceLock<Arc<E2eState>> = OnceLock::new();

/// Lab event table seeded by `/api/test/seed-data` (registered in Spectra catalog).
pub const E2E_EVENT_TABLE: &str = "platform_smoke_event";
/// Event table left empty for empty-table explore scenarios.
pub const E2E_EMPTY_EVENT_TABLE: &str = "e2e.empty_events";
/// Lab metric family seeded by `/api/test/seed-data` (registered in Spectra catalog).
pub const E2E_METRIC_NAME: &str = "platform_smoke_counter";
/// Unseeded metric for empty metric explore scenarios.
pub const E2E_EMPTY_METRIC_NAME: &str = "e2e.empty_metric";

struct HiggsFactory(RouterValenceFactory);

impl HiggsValenceFactory for HiggsFactory {
    fn build(&self, actor_json: &serde_json::Value) -> anyhow::Result<Valence> {
        self.0.build(actor_json).map_err(|e| anyhow::anyhow!("{e}"))
    }
}

fn prepare_env() {
    valence::deletion::register_noop_deletion_dispatcher_for_tests();
    valence::clear_for_test();
    // SAFETY: host boot only.
    unsafe {
        if std::env::var_os("VALENCE_OWNERSHIP_UNIFIED_FETCH").is_none() {
            std::env::set_var("VALENCE_OWNERSHIP_UNIFIED_FETCH", "0");
        }
    }
}

async fn seed_user(id: &str, email_verified: bool, valence: &Valence) {
    let now = Utc::now();
    let confirmed_at = email_verified.then_some(now);
    let user = lepton::generated::User::new(
        Some(lepton::generated::UserUserType::Person),
        Some("e2e-password-hash".to_string()),
        Some(lepton::generated::UserStatus::Active),
        None,
        None,
        confirmed_at,
        None,
        None,
        now,
        now,
    )
    .expect("build user");
    lepton::generated::User::upsert(id, user, valence, valence::use_!(r#"**Test:** Fixture **User** save for `e2e_valence` so the suite can arrange and assert persistence behavior. CI and developers running the suite only."#))
        .await
        .expect("upsert user");
}

async fn seed_super_user_with_member(system: &Valence, member_user_id: &str) {
    let super_group = gauge::generated::PermissionGroup::new(
        SUPER_USER_GROUP_NAME.to_string(),
        Some("super users".to_string()),
        Utc::now(),
        Utc::now(),
    )
    .expect("build super user group");
    let created =
        gauge::generated::PermissionGroup::upsert("super_user_group", super_group, system, valence::use_!(r#"**Test:** Fixture **Permission Group** save for `e2e_valence` so the suite can arrange and assert persistence behavior. CI and developers running the suite only."#))
            .await
            .expect("upsert super user group");

    let member = lepton::generated::User::get(member_user_id, system, valence::use_!(r#"**Test:** Fixture **User** load for `e2e_valence` so the suite can arrange and assert persistence behavior. CI and developers running the suite only."#))
        .await
        .expect("query member")
        .expect("member exists");
    let principal = gauge::generated::PermissionUserPrincipal::upsert(
        &format!("user:{member_user_id}"),
        gauge::generated::PermissionUserPrincipal::new(
            member.id().expect("member id").clone(),
            member_user_id.to_string(),
        )
        .expect("new principal"),
        system,
        valence::use_!(r#"**Test:** Fixture **Permission User Principal** save for `e2e_valence` so the suite can arrange and assert persistence behavior. CI and developers running the suite only."#),
    )
    .await
    .expect("upsert principal");
    created
        .relate_to_owner_record(
            principal.id().expect("principal id"),
            system,
            valence::use_!(r#"**Test:** Fixture **owner edge** write in `e2e_valence` so the suite can arrange Gauge super-user membership. CI and developers running the suite only."#),
        )
        .await
        .expect("relate super owner");
    created
        .relate_to_member_record(
            principal.id().expect("principal id"),
            system,
            valence::use_!(r#"**Test:** Fixture **member edge** write in `e2e_valence` so the suite can arrange Gauge super-user membership. CI and developers running the suite only."#),
        )
        .await
        .expect("relate super member");
}

async fn demote_admin_from_super_user(system: &Valence) {
    let Some(super_group) = gauge::generated::PermissionGroup::get("super_user_group", system, valence::use_!(r#"**Test:** Fixture **Permission Group** load for `e2e_valence` so the suite can arrange and assert persistence behavior. CI and developers running the suite only."#))
        .await
        .expect("get super user group")
    else {
        return;
    };
    let Some(principal) = gauge::generated::PermissionUserPrincipal::get("user:admin", system, valence::use_!(r#"**Test:** Fixture **Permission User Principal** load for `e2e_valence` so the suite can arrange and assert persistence behavior. CI and developers running the suite only."#))
        .await
        .expect("get admin principal")
    else {
        return;
    };
    let pid = principal.id().expect("principal id").clone();
    let _ = super_group
        .unrelate_from_member_record(
            &pid,
            system,
            valence::use_!(r#"**Test:** Fixture **member edge** remove in `e2e_valence` so the suite can demote a user from super-user after grants. CI and developers running the suite only."#),
        )
        .await;
    let _ = super_group
        .unrelate_from_owner_record(
            &pid,
            system,
            valence::use_!(r#"**Test:** Fixture **owner edge** remove in `e2e_valence` so the suite can demote a user from super-user after grants. CI and developers running the suite only."#),
        )
        .await;
}

fn spectra_query_manifest() -> PermissionManifestInput {
    PermissionManifestInput {
        app_id: "spectra".into(),
        domains: vec![PermissionDomainInput {
            key: "spectra".into(),
            name: "Spectra".into(),
            description: "Spectra log and metric exploration".into(),
            permissions: vec![PermissionInput {
                name: "QueryTable".into(),
                description: "Query Spectra schemas, events, and metrics".into(),
            }],
        }],
    }
}

async fn grant_query_table(admin_ctx: &Valence, user_id: &str) {
    let perms = service::list_permissions(admin_ctx, None)
        .await
        .expect("list permissions");
    let query_table = perms
        .into_iter()
        .find(|p| p.name == "QueryTable")
        .expect("QueryTable after sync");
    service::grant_permission_to_user(&query_table.id, user_id, admin_ctx)
        .await
        .expect("grant QueryTable");
}

fn bootstrap_spectra_fixtures() -> FixtureIds {
    FixtureIds {
        event_table: E2E_EVENT_TABLE.into(),
        empty_event_table: E2E_EMPTY_EVENT_TABLE.into(),
        metric_name: E2E_METRIC_NAME.into(),
        empty_metric_name: E2E_EMPTY_METRIC_NAME.into(),
        seeded_event_count: 3,
    }
}

/// Build shared Valence/Higgs once and seed baseline fixtures.
pub async fn init_e2e_valence() {
    if E2E_STATE.get().is_some() {
        return;
    }

    prepare_env();

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
    let router = Arc::new(router);
    let default_key = router_key(gauge::embedded_surreal::LOGICAL_NAME, MEM_ENGINE_ID);

    let system = Valence::builder()
        .database_router(Arc::clone(&router))
        .default_backend_key(default_key.clone())
        .with_actor(Actor::System {
            operation: "e2e_spectra_host".into(),
        })
        .build()
        .expect("e2e Valence");

    seed_user("admin", true, &system).await;
    seed_user("outsider", true, &system).await;
    seed_user("unverified", false, &system).await;
    seed_super_user_with_member(&system, "admin").await;

    sync_permission_manifests(&system, &[spectra_query_manifest()])
        .await
        .expect("sync Spectra QueryTable manifest");

    let admin_ctx = system.with_actor(Actor::User {
        user_id: "admin".to_string(),
    });
    grant_query_table(&admin_ctx, "admin").await;
    demote_admin_from_super_user(&system).await;

    let fixtures = bootstrap_spectra_fixtures();

    let factory: Arc<dyn HiggsValenceFactory> = Arc::new(HiggsFactory(RouterValenceFactory::new(
        Arc::clone(&router),
        RouterValenceFactoryConfig::new(default_key.clone())
            .actor_json_policy(external_actor_json_policy()),
    )));
    let higgs = Arc::new(
        HiggsConfig::builder()
            .valence_factory_arc(factory)
            .build()
            .expect("e2e HiggsConfig"),
    );

    let state = Arc::new(E2eState {
        router,
        higgs,
        default_backend_key: default_key,
        fixtures: Mutex::new(fixtures),
    });
    let _ = E2E_STATE.set(state);
}

fn state() -> Arc<E2eState> {
    E2E_STATE
        .get()
        .expect("init_e2e_valence must run first")
        .clone()
}

pub fn e2e_router() -> Arc<DatabaseRouter> {
    Arc::clone(&state().router)
}

pub fn e2e_higgs_config() -> Arc<HiggsConfig> {
    Arc::clone(&state().higgs)
}

pub fn e2e_fixtures() -> FixtureIds {
    state().fixtures.lock().expect("fixtures").clone()
}

pub fn e2e_system_valence() -> Valence {
    Valence::builder()
        .database_router(e2e_router())
        .default_backend_key(state().default_backend_key.clone())
        .with_actor(Actor::System {
            operation: "e2e_seed".into(),
        })
        .build()
        .expect("system valence")
}

pub fn e2e_admin_valence() -> Valence {
    e2e_system_valence().with_actor(Actor::User {
        user_id: "admin".into(),
    })
}
