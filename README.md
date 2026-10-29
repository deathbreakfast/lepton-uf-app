# lepton-uf-app

[![CI](https://github.com/unified-field-dev/lepton-uf-app/actions/workflows/ci.yml/badge.svg)](https://github.com/unified-field-dev/lepton-uf-app/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[GitHub](https://github.com/unified-field-dev/lepton-uf-app) · `cargo doc -p lepton-shell --features ssr --open`

Product UI for Lepton on Unified Field hosts: shell avatar menu, `/user` settings, and `/auth` routes. This repository is a set of mountable crates, not a standalone host binary.

```toml
[dependencies]
# Pin tag or rev — do not use branch = "main".
lepton-shell = { git = "https://github.com/unified-field-dev/lepton-uf-app", package = "lepton-shell", rev = "REPLACE_WITH_PIN", default-features = false }
lepton-app = { git = "https://github.com/unified-field-dev/lepton-uf-app", package = "lepton-app", rev = "REPLACE_WITH_PIN", default-features = false }
lepton-auth-app = { git = "https://github.com/unified-field-dev/lepton-uf-app", package = "lepton-auth-app", rev = "REPLACE_WITH_PIN", default-features = false }
```

Depends on [lepton](https://github.com/unified-field-dev/lepton) (`lepton-auth`, `lepton-auth-ui`) and [unified-field-product](https://github.com/unified-field-dev/unified-field-product) (session types, shell slots, `uf_app!` registry).

## Workspace

| Crate | Role |
|-------|------|
| [`lepton-shell`](lepton-shell/) | App-bar auth menu (`AppBarUserMenu`), `AuthDialog`, and `StepUpDialog` mount |
| [`lepton-app`](lepton-app/) | `/user` profile, appearance, account settings, confirm-account |
| [`lepton-auth-app`](lepton-auth-app/) | `/auth` sign-in, sign-up, logout, password reset, OAuth callback |
| [`lepton-uf-app-e2e`](lepton-uf-app-e2e/) | Leptos host + Playwright that mounts all three on `:3140` |

Crate-root rustdoc documents Concern → API tables and route maps. Start at `cargo doc -p lepton-shell --features ssr --open`, then open `lepton-app` / `lepton-auth-app`.

## Mount on a host

1. Depend on the three crates (path or git pin). Enable `ssr` / `hydrate` the same way as other uf-apps.
2. Drop `<UserAppRoutes />` and `<LeptonAuthRoutes />` under the host `Router` / `Routes` (`uf_app!` inside each crate registers `/user` and `/auth`).
3. Call `uf_integrations::provide_shell_auth_menu(|| view! { <AppBarUserMenu /> })` so chrome picks up the menu via `HostAuthMenu`.
4. Provide host session chrome (`get_session` / `init_auth_resource` / `use_auth_context`) the way your product shell already does.
5. At SSR boot, call `lepton_auth::services::provide_auth_services` with email/SMS adapters and `public_base_url` (see lepton crate root).

```rust,ignore
use lepton_app::UserAppRoutes;
use lepton_auth_app::LeptonAuthRoutes;
use lepton_shell::AppBarUserMenu;
use leptos::prelude::*;
use leptos_router::components::{Router, Routes};
use uf_integrations::provide_shell_auth_menu;

provide_shell_auth_menu(|| view! { <AppBarUserMenu /> });

view! {
    <Router>
        <Routes fallback=|| view! { /* host 404 */ }>
            <UserAppRoutes />
            <LeptonAuthRoutes />
        </Routes>
    </Router>
}
// SSR boot: lepton_auth::services::provide_auth_services(...);
```

Copy-paste reference that compiles the three crates: [`lepton-uf-app-e2e/src/app.rs`](lepton-uf-app-e2e/src/app.rs).

Feature flags hosts commonly enable on SSR + hydrate:

| Need | Features |
|------|----------|
| TOTP on Account Settings | `lepton-auth/totp` (already on `lepton-app` `ssr`) |
| OAuth buttons / link | `lepton-app/oauth-google`, `lepton-app/oauth-github` (and matching auth-app / auth-ui features) |
| Passkey UI | `lepton-app/webauthn` + `lepton-auth/webauthn` on SSR |

Env for OAuth / public URLs follows lepton (`UF_PUBLIC_BASE_URL`, `UF_OAUTH_*`, or `UF_OAUTH_USE_MOCK=1`). Signup disable: `UF_LEPTON_SIGNUP_DISABLED=1` (see lepton `SECURITY.md`).

## Account Settings (`/user/account-settings`)

| Concern | UI | Server |
|---------|----|--------|
| Email verify / change password | `AccountSettingsPage` | `lepton_auth::actions::account` |
| Authenticator (TOTP) | `TotpSettingsSection` | `lepton_auth::actions::totp` |
| Connected accounts | `ConnectedAccountsSection` | `lepton_auth::actions::oauth_settings` |
| Security devices | `SecurityDevicesSection` | `lepton_auth::actions::devices` |
| Owner wipe | private `wipe_section` (composed by `AccountSettingsPage`) | `WipeAccount` / `execute_wipe_account` |
| Soft confirm funnel | `/user/confirm-account` | `lepton_auth_ui::ConfirmAccountPage` |

`AppBarUserMenu` mounts `StepUpDialog` so hosts that call `StepUpController::request` get a working dialog. Account Settings uses session auth today.

## Service users (`/user/service-users`)

`lepton-app` lists the signed-in person's service users, with create (profile, access, token), a detail page (groups, permissions, Pause and Resume, Delete) and one-time token reveal and revoke. The host provides a `ServiceUserAccessBackend` for access choices and tokens; without one the pages still list and create service users and say tokens aren't set up. Each person can own a limited number of service users, and the pages show that limit.

## Examples

- [`lepton-mount-host`](examples/lepton-mount-host/) — path/auth/inventory contract smoke (Axum oneshot; does not compile or mount lepton UI crates)
- [`lepton-uf-app-e2e`](lepton-uf-app-e2e/) — real Leptos mount of all three crates + Playwright

```bash
export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR=target-lepton-uf-app
cargo run -p lepton-mount-host
cargo test -p lepton-shell --features ssr --test product_surface
cargo test -p lepton-shell --features ssr --test shell_step_up_surface
```

**Success (mount-host):** stdout prints `lepton_mount_host: OK — /user protect + /auth public + inventory`.
For a full UI host in-tree, see [`lepton-uf-app-e2e`](lepton-uf-app-e2e/). Or start from unified-field-product `shell-chrome-host` and swap the Demo auth menu for `AppBarUserMenu`.

## Security

Auth threat model, operator hardening, and sensitive env vars live in the lepton kit
[`SECURITY.md`](https://github.com/unified-field-dev/lepton/blob/main/SECURITY.md).
This repo mounts UI surfaces over that contract.

### Host duties (this product layer)

| Concern | Contract |
|---------|----------|
| Session cookies | Production hosts set `HttpOnly`, `Secure`, and `SameSite=Lax` or `Strict`. The e2e host uses `with_secure(false)` for localhost only. |
| Signup | Set `UF_LEPTON_SIGNUP_DISABLED=1` when open registration must stay closed. |
| Profile photo upload | `ProfilePhotoUpload` POSTs to `/api/files/upload` and may include a client `profile_id` field. That field is **not** authorization. The host handler must bind the upload to the **session user** (derive or verify ownership against session Valence). Do not use System Valence to attach an arbitrary client `profile_id`. |
| Seed / System factory | Never mount `POST /api/test/seed-data` or copy the e2e Higgs factory that allows System-shaped actors without an external actor policy. Those exist only under [`lepton-uf-app-e2e`](lepton-uf-app-e2e/). |

Profile and account mutations go through `require_auth_user` / kit server fns. UI anon redirects on `/user/*` are hygiene; server fns remain the authz boundary.

## Verify

See [`docs/VERIFICATION.md`](docs/VERIFICATION.md) for check, mount-host, shell tests, rustdoc, and product e2e gates.

CI runs on every push and PR ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)):
fmt, clippy (`-D warnings`), product-crate check + `lepton-mount-host` run, shell surface tests, and rustdoc with `-D warnings` for `lepton-shell`, `lepton-app`, and `lepton-auth-app`. No root `deny.toml`, so deny is not in CI.

```bash
export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR=target-lepton-uf-app
cargo check -p lepton-shell -p lepton-app -p lepton-auth-app --features ssr
cargo check -p lepton-mount-host
cargo run -p lepton-mount-host
cargo test -p lepton-shell --features ssr --test workspace_members
cargo test -p lepton-shell --features ssr --test shell_step_up_surface
cargo test -p lepton-shell --features ssr --test product_surface
RUSTDOCFLAGS="-D warnings" cargo doc -p lepton-shell -p lepton-app -p lepton-auth-app --features ssr --no-deps
```

See [`docs/VERIFICATION.md`](docs/VERIFICATION.md) for product e2e and leptos-lints.

## FAQ

**Is this a runnable host?** The product crates are mountable libraries. Contract smoke: `cargo run -p lepton-mount-host`. Full Leptos mount in this repo: `lepton-uf-app-e2e`. A product binary still needs session chrome and `provide_auth_services`.

**Where does identity logic live?** In lepton (`lepton-auth` / higgs). These crates ship Orbital UI and `uf_app!` route registration atop that contract.

## License

MIT (`license` in workspace `Cargo.toml`).
