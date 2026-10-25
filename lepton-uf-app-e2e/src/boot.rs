//! Valence + Higgs boot for the product e2e host.
//!
//! Uses tolerant in-memory Valence (same pattern as `lepton-e2e` / auth integration
//! tests). `SQLite` rejects Surreal-shaped `SELECT VALUE` unique probes.
//!
//! Mail goes to Mailpit on `127.0.0.1:1025`, SMS to the `lepton-e2e` sink on `:8099`,
//! and OAuth to the mock IdP on `:5556`. Delivery is synchronous: no Boson runtime is
//! installed here, so Playwright can read a code straight after the action.
//!
//! Lab-only: System-shaped actors without `external_actor_json_policy`, ownership
//! env forced off, and tolerant unique-index behavior. Do not copy this factory
//! into production hosts.

use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use higgs::{HiggsConfig, HiggsValenceFactory};
use lepton_auth::services::LeptonAuthServicesBuilder;
use lepton_host_adapter::Backend;
use valence::{
    register_backend_logical_names_slices, router_key, Actor, CompiledQuery, DatabaseBackend,
    DatabaseRouter, InMemoryBackend, RecordId, RegisterBackendLogicalNamesOptions, Valence,
    MEM_ENGINE_ID, SQLITE_ENGINE_ID,
};

/// Shared process state for Axum + Leptos.
#[derive(Clone)]
pub struct AppState {
    /// Leptos configuration.
    pub leptos_options: leptos::config::LeptosOptions,
    /// Higgs boot config (Valence factory).
    pub higgs: Arc<HiggsConfig>,
    /// Valence router (also Axum `Extension`).
    pub valence_router: Arc<DatabaseRouter>,
    /// Default backend key (`mem`).
    pub default_backend_key: String,
}

impl axum::extract::FromRef<AppState> for leptos::config::LeptosOptions {
    fn from_ref(state: &AppState) -> Self {
        state.leptos_options.clone()
    }
}

struct MemHiggsFactory {
    router: Arc<DatabaseRouter>,
    default_backend_key: String,
}

impl HiggsValenceFactory for MemHiggsFactory {
    fn build(&self, actor_json: &serde_json::Value) -> anyhow::Result<Valence> {
        // Internal trust: allow System-shaped actors for `unsafe_system_valence`
        // (signup / reset). Do not install `external_actor_json_policy` here.
        let actor: Actor = serde_json::from_value(actor_json.clone())
            .map_err(|e| anyhow::anyhow!("actor deserialize: {e}"))?;
        Valence::builder()
            .database_router(Arc::clone(&self.router))
            .default_backend_key(self.default_backend_key.clone())
            .with_actor(actor)
            .build()
            .map_err(|e| anyhow::anyhow!("{e}"))
    }
}

/// Wraps [`InMemoryBackend`] and treats unsupported unique-index DDL as success.
#[derive(Debug)]
struct TolerantMemBackend {
    inner: InMemoryBackend,
}

impl TolerantMemBackend {
    fn new() -> Self {
        Self {
            inner: InMemoryBackend::new(),
        }
    }
}

#[async_trait]
impl DatabaseBackend for TolerantMemBackend {
    fn engine_id(&self) -> &'static str {
        self.inner.engine_id()
    }

    fn capabilities(&self) -> valence::BackendCapabilities {
        self.inner.capabilities()
    }

    async fn use_namespace(&self, ns: &str, db_name: &str) -> valence::Result<()> {
        self.inner.use_namespace(ns, db_name).await
    }

    async fn execute_compiled_query(
        &self,
        compiled: &CompiledQuery,
    ) -> valence::Result<Vec<serde_json::Value>> {
        let rows = self.inner.execute_compiled_query(compiled).await?;
        if compiled
            .query_string
            .to_ascii_uppercase()
            .contains("SELECT VALUE")
        {
            return Ok(rows
                .into_iter()
                .map(|row| match row {
                    serde_json::Value::Object(mut obj)
                        if obj.len() == 1 && obj.contains_key("id") =>
                    {
                        obj.remove("id").unwrap_or(serde_json::Value::Null)
                    }
                    other => other,
                })
                .collect());
        }
        Ok(rows)
    }

    async fn ensure_schemaless_table(&self, table: &str) -> valence::Result<()> {
        self.inner.ensure_schemaless_table(table).await
    }

    async fn get_record(
        &self,
        table: &str,
        id: &str,
    ) -> valence::Result<Option<serde_json::Value>> {
        self.inner.get_record(table, id).await
    }

    async fn create_record(
        &self,
        table: &str,
        content: serde_json::Value,
    ) -> valence::Result<serde_json::Value> {
        self.inner.create_record(table, content).await
    }

    async fn update_record(
        &self,
        table: &str,
        id: &str,
        content: serde_json::Value,
    ) -> valence::Result<serde_json::Value> {
        self.inner.update_record(table, id, content).await
    }

    async fn merge_record(
        &self,
        table: &str,
        id: &str,
        patch: serde_json::Value,
    ) -> valence::Result<serde_json::Value> {
        self.inner.merge_record(table, id, patch).await
    }

    async fn upsert_record(
        &self,
        table: &str,
        id: &str,
        content: serde_json::Value,
    ) -> valence::Result<serde_json::Value> {
        self.inner.upsert_record(table, id, content).await
    }

    async fn delete_record(&self, table: &str, id: &str) -> valence::Result<()> {
        self.inner.delete_record(table, id).await
    }

    async fn relate_edge(
        &self,
        from: &RecordId,
        edge_table: &str,
        to: &RecordId,
    ) -> valence::Result<()> {
        self.inner.relate_edge(from, edge_table, to).await
    }

    async fn unrelate_edge(
        &self,
        from: &RecordId,
        edge_table: &str,
        to: &RecordId,
    ) -> valence::Result<()> {
        self.inner.unrelate_edge(from, edge_table, to).await
    }

    async fn get_edge_targets(
        &self,
        from: &RecordId,
        edge_table: &str,
    ) -> valence::Result<Vec<RecordId>> {
        self.inner.get_edge_targets(from, edge_table).await
    }

    async fn define_unique_index(&self, table: &str, field: &str) -> valence::Result<()> {
        match self.inner.define_unique_index(table, field).await {
            Ok(()) => Ok(()),
            // uf-valence-backend-mem 0.1.3+: "unique indexes not supported on in-memory backend"
            Err(valence::Error::Internal(msg))
                if msg.contains("define_unique_index")
                    || msg.contains("unique indexes not supported") =>
            {
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    fn ttl_capability(&self) -> valence::ttl::BackendTtlCapability {
        self.inner.ttl_capability()
    }

    async fn apply_ttl_policy(
        &self,
        table: &str,
        policy: &valence::ttl::SchemaTtlPolicy,
    ) -> valence::Result<()> {
        self.inner.apply_ttl_policy(table, policy).await
    }
}

/// Lab defaults for first-boot env seeding, applied only when unset.
///
/// A lab setting from the environment, or `default` when it's unset or
/// blank. The defaults match a single local host; the lane runner sets one
/// value per lane so each host gets its own mail, SMS and OIDC sidecars.
pub fn lab_env(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}

/// The cell seeds its delivery settings from env once, exactly as a real host
/// does. Mail starts on Mailpit, text messages and SendGrid on the lab mocks in
/// [`crate::mock_providers`], and OAuth on the mock IdP.
fn seed_lab_env_defaults() {
    const DEFAULTS: &[(&str, &str)] = &[
        ("UF_EMAIL_DRIVER", "smtp"),
        ("UF_SMTP_HOST", "127.0.0.1"),
        ("UF_SMTP_PORT", "1025"),
        ("UF_SMTP_USE_TLS", "0"),
        ("UF_EMAIL_FROM", "noreply@example.test"),
        ("UF_EMAIL_FROM_NAME", "Lepton Auth"),
        (
            "UF_SENDGRID_API_BASE_URL",
            "http://127.0.0.1:3140/mock/sendgrid",
        ),
        (
            "UF_TWILIO_API_BASE_URL",
            "http://127.0.0.1:3140/mock/twilio",
        ),
        (
            "UF_TWILIO_ACCOUNT_SID",
            "ACe2e00000000000000000000000000000",
        ),
        ("UF_TWILIO_API_KEY", "SKe2e00000000000000000000000000000"),
        ("UF_TWILIO_API_SECRET", "lab-initial-twilio-secret"),
        (
            "UF_TWILIO_VERIFY_SERVICE_SID",
            "VAe2e00000000000000000000000000000",
        ),
        ("UF_OAUTH_USE_MOCK", "1"),
        // Lab-only vault master key (64 hex zeros). Never reuse outside this host.
        (
            "NEUTRINO_MASTER_KEY",
            "0000000000000000000000000000000000000000000000000000000000000000",
        ),
    ];
    for (key, value) in DEFAULTS {
        if std::env::var_os(key).is_none() {
            std::env::set_var(key, value);
        }
    }
    if std::env::var_os("UF_MOCK_OIDC_URL").is_none() {
        std::env::set_var("UF_MOCK_OIDC_URL", "http://127.0.0.1:5556");
    }
}

/// Open in-memory Valence, register identity, Gauge and Neutrino schemas, install
/// delivery settings, build Higgs + auth services.
///
/// # Errors
///
/// Returns when Higgs, the auth services or delivery settings cannot be built.
#[allow(clippy::too_many_lines)] // platform boot: valence + schemas + Higgs + delivery + auth
pub async fn boot_platform() -> anyhow::Result<(
    AppState,
    Backend,
    Arc<lepton_auth::services::LeptonAuthServices>,
    uf_lepton_delivery_boot::DeliveryBoot,
)> {
    // Force-link identity inventory schemas.
    let _ = lepton_identity::generated::UserStatus::Active;

    if std::env::var_os("VALENCE_OWNERSHIP_UNIFIED_FETCH").is_none() {
        std::env::set_var("VALENCE_OWNERSHIP_UNIFIED_FETCH", "0");
    }
    if std::env::var_os("VALENCE_OWNERSHIP_COLOCATE").is_none() {
        std::env::set_var("VALENCE_OWNERSHIP_COLOCATE", "0");
    }
    // Sealed TOTP secrets require a key; CI/local e2e use the non-production test key.
    if std::env::var_os("LEPTON_TOTP_SEAL_KEY").is_none()
        && std::env::var_os("LEPTON_TOTP_ALLOW_TEST_SEAL_KEY").is_none()
    {
        std::env::set_var("LEPTON_TOTP_ALLOW_TEST_SEAL_KEY", "1");
    }
    seed_lab_env_defaults();

    let backend: Arc<dyn DatabaseBackend> = Arc::new(TolerantMemBackend::new());
    let mut router = DatabaseRouter::new();
    // Identity schemas declare the SQLite engine id; Gauge and Neutrino keep their own
    // logical names. One tolerant mem backend serves all of them.
    register_backend_logical_names_slices(
        &mut router,
        Arc::clone(&backend),
        &[
            &["default"],
            neutrino::embedded_surreal::EMBEDDED_SURREAL_LOGICAL_NAMES,
        ],
        RegisterBackendLogicalNamesOptions {
            register_alias_engine_id: Some(SQLITE_ENGINE_ID),
        },
    );
    gauge::embedded_surreal::register_storage(&mut router, backend);
    let default_backend_key = router_key("default", MEM_ENGINE_ID);
    let valence_router = Arc::new(router);

    let factory: Arc<dyn HiggsValenceFactory> = Arc::new(MemHiggsFactory {
        router: Arc::clone(&valence_router),
        default_backend_key: default_backend_key.clone(),
    });
    let higgs = Arc::new(
        HiggsConfig::builder()
            .valence_factory_arc(Arc::clone(&factory))
            .build()
            .context("HiggsConfig")?,
    );

    // Gauge permissions, then delivery settings, under the System Valence the host booted with.
    gauge::touch_schema_inventory();
    let system = system_valence(Arc::clone(&valence_router), &default_backend_key)?;
    system
        .sync_typed_tables_from_registry()
        .await
        .map_err(|e| anyhow::anyhow!("sync typed tables: {e}"))?;
    crate::permissions::sync_registered_manifests(&system)
        .await
        .context("permission manifest sync")?;
    // The Playwright origin. A lane runner sets it per lane.
    let public_base_url = lab_env("UF_PUBLIC_BASE_URL", "http://localhost:3140");
    let delivery = uf_lepton_delivery_boot::install_delivery(
        &system,
        uf_lepton_delivery_boot::DeliveryOptions::new(&public_base_url),
    )
    .await
    .context("install delivery settings")?;

    let auth_services = Arc::new(
        LeptonAuthServicesBuilder::new()
            .email(delivery.email())
            .sms(delivery.sms())
            .public_base_url(public_base_url.clone())
            .oauth_handle(delivery.oauth())
            .webauthn_rp(lepton_auth::devices::WebauthnRpConfig {
                // Match the Playwright origin (`UF_PUBLIC_BASE_URL`).
                rp_id: "localhost".into(),
                rp_origin: public_base_url,
                rp_name: "Lepton".into(),
            })
            .build()?,
    );

    let auth_backend = Backend::new(factory);

    let conf = leptos::config::get_configuration(Some("Cargo.toml"))
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let state = AppState {
        leptos_options: conf.leptos_options,
        higgs,
        valence_router,
        default_backend_key,
    };
    Ok((state, auth_backend, auth_services, delivery))
}

/// System Valence for seed / bootstrap.
pub fn system_valence(
    router: Arc<DatabaseRouter>,
    default_backend_key: &str,
) -> anyhow::Result<Valence> {
    Valence::builder()
        .database_router(router)
        .default_backend_key(default_backend_key.to_owned())
        .with_actor(Actor::System {
            operation: "lepton_uf_app_e2e".to_string(),
        })
        .build()
        .map_err(|e| anyhow::anyhow!("{e}"))
}
