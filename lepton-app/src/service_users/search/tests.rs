//! Privacy and filtering tests for the service user search source, on in-memory Valence.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::{Arc, Once};

use async_trait::async_trait;
use chrono::Utc;
use lepton_auth::service_users::{create_service_user, delete_service_user, NewServiceUser};
use lepton_identity::generated::{User, UserStatus, UserUserType};
use lepton_identity::service_users::{
    install_service_user_gate, ServiceUserGate, ServiceUserGateError, ServiceUserId,
};
use uf_search_core::{SearchSourceItem, SearchSourceProvider};
use valence::{
    register_backend_logical_names_slices, router_key, Actor, DatabaseRouter, InMemoryBackend,
    Model, RegisterBackendLogicalNamesOptions, Valence, MEM_ENGINE_ID,
};

use super::{ServiceUserSearchSource, SERVICE_USER_SEARCH_SOURCE_ID};

const MAKER: &str = "maker";
const SUPER: &str = "super";

/// Decides from the user id prefix, so parallel tests share one installed gate.
struct PrefixGate;

fn user_id(v: &Valence) -> String {
    v.actor().user_id().unwrap_or_default().to_string()
}

#[async_trait]
impl ServiceUserGate for PrefixGate {
    async fn may_create_service_user(&self, v: &Valence) -> Result<bool, ServiceUserGateError> {
        let id = user_id(v);
        Ok(id.starts_with(MAKER) || id.starts_with(SUPER))
    }

    async fn may_manage_any_service_user(&self, v: &Valence) -> Result<bool, ServiceUserGateError> {
        Ok(user_id(v).starts_with(SUPER))
    }
}

fn setup() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        install_service_user_gate(Arc::new(PrefixGate)).expect("first gate install");
        valence::deletion::register_noop_deletion_dispatcher_for_tests();
    });
}

/// A fresh in-memory Valence acting as `actor`; every call is an isolated store.
fn store(actor: Actor) -> Valence {
    std::env::set_var("VALENCE_OWNERSHIP_UNIFIED_FETCH", "0");
    std::env::set_var("VALENCE_OWNERSHIP_COLOCATE", "0");
    let mut router = DatabaseRouter::new();
    register_backend_logical_names_slices(
        &mut router,
        Arc::new(InMemoryBackend::new()),
        &[&["default"]],
        RegisterBackendLogicalNamesOptions::default(),
    );
    Valence::builder()
        .database_router(Arc::new(router))
        .default_backend_key(router_key("default", MEM_ENGINE_ID))
        .with_actor(actor)
        .build()
        .expect("valence")
}

fn as_user(base: &Valence, id: &str) -> Valence {
    base.with_actor(Actor::User {
        user_id: id.to_string(),
    })
}

/// **Test:** seed an active person user with a chosen id.
async fn seed_user(system: &Valence, id: &str) {
    let now = Utc::now();
    let user = User::new(
        Some(UserUserType::Person),
        None,
        Some(UserStatus::Active),
        None,
        None,
        None,
        None,
        None,
        now,
        now,
    )
    .expect("user");
    User::upsert(id, user, system, valence::use_!(r"**Test:** Fixture **User** save for the service user search tests so they can create service users owned by a known user id. CI and developers running the suite only."))
        .await
        .expect("seed user");
}

/// Create a service user owned by `owner`; returns its bare id.
async fn seed_service_user(
    system: &Valence,
    owner: &str,
    handle: &str,
    display_name: &str,
) -> ServiceUserId {
    seed_user(system, owner).await;
    let created = create_service_user(
        &as_user(system, owner),
        NewServiceUser {
            handle,
            display_name,
            description: None,
        },
    )
    .await
    .expect("owner creates service user");
    ServiceUserId::parse(created.id().expect("id").id()).expect("service user id")
}

async fn search(v: &Valence, text: &str, max: u32) -> Vec<SearchSourceItem> {
    ServiceUserSearchSource
        .query(v, text, max)
        .await
        .expect("search")
}

fn titles(items: &[SearchSourceItem]) -> Vec<&str> {
    let mut t: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
    t.sort_unstable();
    t
}

#[tokio::test]
async fn owner_sees_own_service_users_with_name_and_owner() {
    setup();
    let system = store(Actor::System {
        operation: "test".to_string(),
    });
    let id = seed_service_user(&system, "maker-a", "ci-bot", "CI bot").await;

    let items = search(&as_user(&system, "maker-a"), "", 10).await;

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "CI bot");
    assert_eq!(items[0].id, id.as_str());
    assert_eq!(items[0].source_id, SERVICE_USER_SEARCH_SOURCE_ID);
    assert_eq!(items[0].kind, "service_user");
    assert_eq!(items[0].description.as_deref(), Some("Owned by you"));
}

#[tokio::test]
async fn stranger_sees_none_of_another_owners_service_users() {
    setup();
    let system = store(Actor::System {
        operation: "test".to_string(),
    });
    seed_service_user(&system, "maker-a", "a-bot", "A bot").await;
    seed_service_user(&system, "maker-b", "b-bot", "B bot").await;
    seed_user(&system, "member-c").await;

    assert_eq!(
        titles(&search(&as_user(&system, "maker-a"), "", 10).await),
        ["A bot"]
    );
    assert_eq!(
        titles(&search(&as_user(&system, "maker-b"), "", 10).await),
        ["B bot"]
    );
    assert_eq!(search(&as_user(&system, "member-c"), "", 10).await.len(), 0);
    assert_eq!(
        search(&as_user(&system, "member-c"), "bot", 10).await.len(),
        0
    );
}

#[tokio::test]
async fn manage_any_sees_every_owners_service_users() {
    setup();
    let system = store(Actor::System {
        operation: "test".to_string(),
    });
    seed_service_user(&system, "maker-a", "a-bot", "A bot").await;
    seed_service_user(&system, "maker-b", "b-bot", "B bot").await;
    seed_user(&system, "super-s").await;

    let items = search(&as_user(&system, "super-s"), "", 10).await;

    assert_eq!(titles(&items), ["A bot", "B bot"]);
    assert!(items.iter().all(|i| i
        .description
        .as_deref()
        .is_some_and(|d| d.starts_with("Owned by "))));
    assert!(items
        .iter()
        .all(|i| i.description.as_deref() != Some("Owned by you")));
}

#[tokio::test]
async fn service_user_actor_gets_nothing() {
    setup();
    let system = store(Actor::System {
        operation: "test".to_string(),
    });
    let id = seed_service_user(&system, "maker-a", "a-bot", "A bot").await;

    let as_service_user = system.with_actor(Actor::ServiceUser {
        service_name: format!("svc:{}", id.as_str()),
    });

    assert_eq!(search(&as_service_user, "", 10).await.len(), 0);
    assert_eq!(
        search(&system.with_actor(Actor::Anonymous), "", 10)
            .await
            .len(),
        0
    );
    assert_eq!(search(&system, "", 10).await.len(), 0);
}

#[tokio::test]
async fn deleted_service_user_is_hidden_while_deletion_runs() {
    setup();
    let system = store(Actor::System {
        operation: "test".to_string(),
    });
    let gone = seed_service_user(&system, "maker-a", "gone-bot", "Gone bot").await;
    seed_service_user(&system, "maker-a", "kept-bot", "Kept bot").await;
    let owner = as_user(&system, "maker-a");
    delete_service_user(&owner, &gone).await.expect("delete");

    assert_eq!(titles(&search(&owner, "", 10).await), ["Kept bot"]);
    assert_eq!(
        titles(&search(&as_user(&system, "super-s"), "", 10).await),
        ["Kept bot"]
    );
}

#[tokio::test]
async fn query_text_filters_by_name_or_handle_and_limit_applies() {
    setup();
    let system = store(Actor::System {
        operation: "test".to_string(),
    });
    seed_service_user(&system, "maker-a", "alpha-bot", "Builder").await;
    seed_service_user(&system, "maker-a", "beta-bot", "Reporter").await;
    seed_service_user(&system, "maker-a", "gamma-bot", "Reporter two").await;
    let owner = as_user(&system, "maker-a");

    assert_eq!(titles(&search(&owner, "BUILD", 10).await), ["Builder"]);
    assert_eq!(titles(&search(&owner, "beta", 10).await), ["Reporter"]);
    assert_eq!(search(&owner, "reporter", 10).await.len(), 2);
    assert_eq!(search(&owner, "", 2).await.len(), 2);
    assert_eq!(search(&owner, "", 0).await.len(), 0);
    assert_eq!(search(&owner, "nothing-matches", 10).await.len(), 0);
}

#[tokio::test]
async fn search_returns_matches_beyond_the_old_window() {
    setup();
    let system = store(Actor::System {
        operation: "test".to_string(),
    });
    // More readable service users than the old fixed 50-row scan window, so a bounded
    // window would silently drop matches. One per owner keeps each under the quota.
    for i in 0..60 {
        seed_service_user(
            &system,
            &format!("maker-fill-{i}"),
            &format!("fill-{i}"),
            "Filler",
        )
        .await;
    }
    for name in ["pick-a", "pick-b", "pick-c"] {
        seed_service_user(&system, &format!("maker-{name}"), name, "Zzz pick target").await;
    }
    seed_user(&system, "super-s").await;

    // A manage-any administrator reads every owner's service users; all three matches
    // come back even though they sit past the old window.
    let items = search(&as_user(&system, "super-s"), "pick", 10).await;

    assert_eq!(
        titles(&items),
        ["Zzz pick target", "Zzz pick target", "Zzz pick target"]
    );
}

#[test]
fn source_id_matches_the_registered_key() {
    use super::ServiceUserSearchSourceId;

    let key = ServiceUserSearchSourceId::ServiceUser.key();
    assert_eq!(key.id, SERVICE_USER_SEARCH_SOURCE_ID);
    assert_eq!(key.label, "Service users");
}
