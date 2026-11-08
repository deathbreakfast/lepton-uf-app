//! Matching, privacy and limit tests for the people search source, on in-memory Valence.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::{Arc, Once};
use std::time::{Duration, Instant};

use chrono::Utc;
use lepton_identity::generated::{User, UserProfile, UserStatus, UserUserType};
use uf_search_core::{
    SearchSourceItem, SearchSourceKey, SearchSourceProvider, SearchSourceRegistry,
};
use valence::{
    register_backend_logical_names_slices, router_key, Actor, DatabaseRouter, InMemoryBackend,
    Model, RecordId, RegisterBackendLogicalNamesOptions, Valence, MEM_ENGINE_ID,
};

use super::rate_limit::{Bucket, RateLimiter, BURST};
use super::rules::{looks_like_email, match_rank, short_id, SearchOutcome, MAX_PEOPLE};
use super::{PeopleSearchSource, PeopleSearchSourceId};
use crate::people::sources;

/// No results, for `assert_eq!` against a search.
const NONE: [SearchSourceItem; 0] = [];

const ADA: &str = "ada0001aaaa";
const GRACE: &str = "grace002bbbb";
const ALAN: &str = "alan0003cccc";

fn setup() {
    static ONCE: Once = Once::new();
    ONCE.call_once(valence::deletion::register_noop_deletion_dispatcher_for_tests);
}

/// A fresh in-memory store acting as System; every call is an isolated store.
fn store() -> Valence {
    setup();
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
        .with_actor(Actor::System {
            operation: "test".to_string(),
        })
        .build()
        .expect("valence")
}

fn as_user(base: &Valence, id: &str) -> Valence {
    base.with_actor(Actor::User {
        user_id: id.to_string(),
    })
}

/// **Test:** one seeded person: a User row and a profile.
struct Person<'a> {
    id: &'a str,
    legal_name: &'a str,
    display_name: &'a str,
    status: UserStatus,
}

impl<'a> Person<'a> {
    fn new(id: &'a str, display_name: &'a str) -> Self {
        Self {
            id,
            legal_name: "Legal Name Kept Private",
            display_name,
            status: UserStatus::Active,
        }
    }
}

async fn seed(system: &Valence, p: Person<'_>) {
    let now = Utc::now();
    let user = User::new(
        Some(UserUserType::Person),
        None,
        Some(p.status),
        None,
        None,
        None,
        None,
        None,
        now,
        now,
    )
    .expect("user");
    User::upsert(p.id, user, system, valence::use_!(r"**Test:** Fixture **User** save for the people search tests so profiles point at a known account. CI and developers running the suite only."))
        .await
        .expect("seed user");

    let profile_id = format!("profile{}", p.id);
    let profile = UserProfile::new(
        RecordId::new("user", p.id),
        p.legal_name.to_string(),
        p.display_name.to_string(),
        now,
        now,
        None,
    )
    .expect("profile");
    UserProfile::upsert(&profile_id, profile, system, valence::use_!(r"**Test:** Fixture **User Profile** save for the people search tests so a display name can be searched. CI and developers running the suite only."))
        .await
        .expect("seed profile");
}

async fn search(v: &Valence, text: &str, max: u32) -> Vec<SearchSourceItem> {
    PeopleSearchSource
        .query(v, text, max)
        .await
        .expect("search")
}

fn titles(items: &[SearchSourceItem]) -> Vec<&str> {
    items.iter().map(|i| i.title.as_str()).collect()
}

async fn three_people() -> Valence {
    let system = store();
    seed(&system, Person::new(ADA, "Ada Lovelace")).await;
    seed(&system, Person::new(GRACE, "Grace Hopper")).await;
    seed(&system, Person::new(ALAN, "Alan Turing")).await;
    system
}

// --- pure helpers ---

#[test]
fn match_rank_prefers_whole_name_prefix_then_word_prefix() {
    assert_eq!(match_rank("ada", "Ada Lovelace"), Some(0));
    assert_eq!(match_rank("lovelace", "Ada Lovelace"), Some(1));
    assert_eq!(match_rank("ada lov", "Ada Lovelace"), Some(0));
    assert_eq!(match_rank("lov ada", "Ada Lovelace"), Some(1));
    assert_eq!(match_rank("hop", "Grace O'Hopper"), Some(1));
}

#[test]
fn match_rank_rejects_mid_word_and_partial_multi_word_matches() {
    assert_eq!(match_rank("ove", "Ada Lovelace"), None);
    assert_eq!(match_rank("ada hop", "Ada Lovelace"), None);
    assert_eq!(match_rank("", "Ada Lovelace"), None);
}

#[test]
fn short_id_takes_six_characters_or_the_whole_id() {
    assert_eq!(short_id("ada0001aaaa"), "ada000");
    assert_eq!(short_id("abc"), "abc");
}

#[test]
fn looks_like_email_flags_at_signs_only() {
    assert!(looks_like_email("ada@example.test"));
    assert!(!looks_like_email("Ada Lovelace"));
}

#[test]
fn outcome_labels_are_stable() {
    let all = [
        (SearchOutcome::Found, "ok"),
        (SearchOutcome::Empty, "empty"),
        (SearchOutcome::ShortQuery, "short_query"),
        (SearchOutcome::RateLimited, "rate_limited"),
        (SearchOutcome::Anonymous, "anonymous"),
        (SearchOutcome::Error, "error"),
    ];
    for (outcome, label) in all {
        assert_eq!(outcome.label(), label);
    }
}

#[test]
fn registered_id_matches_the_exported_constant() {
    assert_eq!(PeopleSearchSourceId::People.key().id, sources::PEOPLE);
    assert_ne!(sources::PEOPLE, sources::SERVICE_USERS);
    assert_eq!(sources::SERVICE_USERS, "lepton_service_users");
    assert_eq!(sources::TEAMS, "polaron_team");
}

#[test]
fn bucket_allows_the_burst_then_refuses() {
    let now = Instant::now();
    let mut bucket = Bucket::full(3.0, now);
    assert!(bucket.take(3.0, 1.0, now));
    assert!(bucket.take(3.0, 1.0, now));
    assert!(bucket.take(3.0, 1.0, now));
    assert!(!bucket.take(3.0, 1.0, now));
}

#[test]
fn bucket_refills_over_time_but_not_past_capacity() {
    let now = Instant::now();
    let mut bucket = Bucket::full(2.0, now);
    assert!(bucket.take(2.0, 1.0, now));
    assert!(bucket.take(2.0, 1.0, now));
    assert!(!bucket.take(2.0, 1.0, now + Duration::from_millis(500)));
    assert!(bucket.take(2.0, 1.0, now + Duration::from_secs(2)));
    let later = now + Duration::from_secs(60);
    assert!(bucket.take(2.0, 1.0, later));
    assert!(bucket.take(2.0, 1.0, later));
    assert!(!bucket.take(2.0, 1.0, later));
}

#[test]
fn limiter_keeps_separate_buckets_per_person() {
    let limiter = RateLimiter::new(1.0, 0.0);
    let now = Instant::now();
    assert!(limiter.try_take("a", now));
    assert!(!limiter.try_take("a", now));
    assert!(limiter.try_take("b", now));
}

// --- the source on in-memory Valence ---

#[tokio::test]
async fn one_character_query_returns_nothing_and_two_characters_match() {
    let system = three_people().await;
    let viewer = as_user(&system, "viewer-short");

    assert_eq!(search(&viewer, "a", 10).await, NONE);
    assert_eq!(search(&viewer, "  a  ", 10).await, NONE);
    assert_eq!(titles(&search(&viewer, "ad", 10).await), ["Ada Lovelace"]);
}

#[tokio::test]
async fn matches_first_and_last_word_ignoring_case() {
    let system = three_people().await;
    let viewer = as_user(&system, "viewer-words");

    assert_eq!(titles(&search(&viewer, "GRA", 10).await), ["Grace Hopper"]);
    assert_eq!(
        titles(&search(&viewer, "hopper", 10).await),
        ["Grace Hopper"]
    );
    assert_eq!(titles(&search(&viewer, "tur", 10).await), ["Alan Turing"]);
}

#[tokio::test]
async fn mid_word_text_matches_nobody() {
    let system = three_people().await;
    let viewer = as_user(&system, "viewer-mid");

    assert_eq!(search(&viewer, "ove", 10).await, NONE);
    assert_eq!(search(&viewer, "zzz", 10).await, NONE);
}

#[tokio::test]
async fn whole_name_prefix_sorts_before_word_matches() {
    let system = store();
    seed(&system, Person::new("lin00001", "Ada Lin")).await;
    seed(&system, Person::new("lin00002", "Lin Ada")).await;
    let viewer = as_user(&system, "viewer-order");

    assert_eq!(
        titles(&search(&viewer, "lin", 10).await),
        ["Lin Ada", "Ada Lin"]
    );
}

#[tokio::test]
async fn result_shape_is_name_short_id_and_user_kind() {
    let system = three_people().await;
    let items = search(&as_user(&system, "viewer-shape"), "ada", 10).await;

    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item.source_id, sources::PEOPLE);
    assert_eq!(item.id, ADA);
    assert_eq!(item.title, "Ada Lovelace");
    assert_eq!(item.description.as_deref(), Some("ada000"));
    assert_eq!(item.kind, "user");
}

#[tokio::test]
async fn caps_at_max_results_and_at_fifty() {
    let system = store();
    let ids: Vec<String> = (0..60).map(|n| format!("bulk{n:04}xx")).collect();
    for (n, id) in ids.iter().enumerate() {
        seed(&system, Person::new(id, &format!("Sam {n:02}"))).await;
    }
    let viewer = as_user(&system, "viewer-cap");

    assert_eq!(search(&viewer, "sam", 5).await.len(), 5);
    assert_eq!(search(&viewer, "sam", 100).await.len(), MAX_PEOPLE);
    assert_eq!(search(&viewer, "sam", 0).await, NONE);
}

#[tokio::test]
async fn anonymous_and_system_callers_get_nothing() {
    let system = three_people().await;

    assert_eq!(search(&system, "ada", 10).await, NONE);
    assert_eq!(
        search(&system.with_actor(Actor::Anonymous), "ada", 10).await,
        NONE
    );
    assert_eq!(
        search(&as_user(&system, "viewer-anon"), "ada", 10)
            .await
            .len(),
        1
    );
}

#[tokio::test]
async fn no_email_or_legal_name_in_any_field() {
    let system = store();
    seed(
        &system,
        Person {
            legal_name: "Augusta Ada King",
            ..Person::new(ADA, "Ada Lovelace")
        },
    )
    .await;
    // An older profile seeded from the account email.
    seed(&system, Person::new(GRACE, "grace@example.test")).await;
    let viewer = as_user(&system, "viewer-email");

    let items = search(&viewer, "ada", 10).await;
    assert_eq!(titles(&items), ["Ada Lovelace"]);
    for item in &items {
        let fields = [Some(item.title.as_str()), item.description.as_deref()];
        for field in fields.into_iter().flatten() {
            assert!(!field.contains('@'), "email in {field:?}");
            assert!(!field.contains("Augusta"), "legal name in {field:?}");
        }
    }
    assert_eq!(search(&viewer, "grace", 10).await, NONE);
    assert_eq!(search(&viewer, "augusta", 10).await, NONE);
}

#[tokio::test]
async fn own_disabled_or_unverified_account_is_left_out() {
    let system = store();
    seed(
        &system,
        Person {
            status: UserStatus::Disabled,
            ..Person::new(ADA, "Ada Lovelace")
        },
    )
    .await;
    seed(
        &system,
        Person {
            status: UserStatus::PendingVerification,
            ..Person::new(GRACE, "Grace Hopper")
        },
    )
    .await;
    seed(&system, Person::new(ALAN, "Alan Turing")).await;

    assert_eq!(search(&as_user(&system, ADA), "ada", 10).await, NONE);
    assert_eq!(search(&as_user(&system, GRACE), "grace", 10).await, NONE);
    assert_eq!(
        titles(&search(&as_user(&system, ALAN), "alan", 10).await),
        ["Alan Turing"]
    );
}

#[tokio::test]
#[ignore = "needs lepton follow-up: User.status readable by signed-in users"]
async fn another_users_disabled_account_is_left_out() {
    let system = store();
    seed(
        &system,
        Person {
            status: UserStatus::Disabled,
            ..Person::new(ADA, "Ada Lovelace")
        },
    )
    .await;

    assert_eq!(
        search(&as_user(&system, "viewer-suspended"), "ada", 10).await,
        NONE
    );
}

#[tokio::test]
async fn user_whose_deletion_is_running_is_left_out() {
    let system = three_people().await;
    User::delete(ADA, &system, valence::use_!(r"**Test:** Fixture **User** delete for the people search tests so a deletion is left pending. CI and developers running the suite only."))
        .await
        .expect("delete");
    let viewer = as_user(&system, "viewer-deleted");

    assert_eq!(search(&viewer, "ada", 10).await, NONE);
    assert_eq!(
        titles(&search(&viewer, "grace", 10).await),
        ["Grace Hopper"]
    );
}

#[tokio::test]
async fn searches_past_the_burst_are_refused() {
    let system = three_people().await;
    let viewer = as_user(&system, "viewer-burst");
    for _ in 0..BURST {
        search(&viewer, "ada", 10).await;
    }

    let refused = PeopleSearchSource.query(&viewer, "ada", 10).await;
    assert!(refused.is_err());
    // Short queries don't spend searches, and another person still has theirs.
    assert_eq!(search(&viewer, "a", 10).await, NONE);
    assert_eq!(
        search(&as_user(&system, "viewer-other"), "ada", 10)
            .await
            .len(),
        1
    );
}

#[tokio::test]
async fn registry_routes_the_people_id_to_this_source() {
    let system = three_people().await;
    let viewer = as_user(&system, "viewer-registry");
    let registry = SearchSourceRegistry::auto_discover();

    let keys = [SearchSourceKey::new(sources::PEOPLE, "People")];
    let hits = registry
        .query_many(&keys, &viewer, "grace", 10)
        .await
        .expect("query_many");
    assert_eq!(titles(&hits), ["Grace Hopper"]);

    let typo = [SearchSourceKey::new("lepton_peoples", "People")];
    assert_eq!(
        registry
            .query_many(&typo, &viewer, "grace", 10)
            .await
            .expect("unknown id"),
        NONE
    );
}
