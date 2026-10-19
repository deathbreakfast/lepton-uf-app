# lepton-uf-app-e2e

Leptos host + Playwright for the product crates in this workspace:
[`lepton-shell`](../lepton-shell/), [`lepton-app`](../lepton-app/), and
[`lepton-auth-app`](../lepton-auth-app/).

Nothing here is a fixture. `src/app.rs` mounts `UserAppRoutes`, `LeptonAuthRoutes`,
and `AppBarUserMenu` through the `uf-integrations` shell slot, the same way a product
binary does. If a spec passes against this host, the composition a host would ship
works.

Boot uses tolerant in-memory Valence, because SQLite cannot run the Surreal-shaped
unique probes that identity schemas emit. The Higgs SSR factory allows internal
System minting for signup and password reset, matching
`ProcessValenceFactory::as_higgs_factory` on the embedded host.

**Do not copy this boot into a production host.** Lab-only choices that must stay here:

- `SessionManagerLayer.with_secure(false)` (localhost cookies)
- `POST /api/test/seed-data` (System Valence seed)
- Higgs factory that allows System-shaped actors without `external_actor_json_policy`
- Forced `VALENCE_OWNERSHIP_*=0` and the tolerant mem backend

Production hosts need `Secure` cookies, no seed route, and a factory policy that
matches the kit SECURITY.md guidance.

## Ports and sidecars

Playwright drives `http://localhost:3140` (WebAuthn RP id `localhost`) while the
process binds `127.0.0.1:3140` (see `src/boot.rs` / `end2end/playwright.config.ts`
`PLAYWRIGHT_BASE_URL`). Live reload uses the leptos reload port beside that bind.
Keeps clear of product hosts on `:3000`–`:3001` and the lepton kit harness on
`:3120`–`:3121`.

On boot the host starts these, or reuses them when something is already listening:

| Sidecar | Bind | Role |
|---------|------|------|
| Mock OIDC | `127.0.0.1:5556` | OAuth IdP for sign-in and account linking |
| SMS HTTP sink | `127.0.0.1:8099` | Captures SMS OTP (`HttpCaptureSmsAdapter`) |

Email goes to **Mailpit** (SMTP `1025`, API `8025`), which you have to start
yourself. Anything that reads a code out of email needs it up first: the confirm
funnel, email change, and password reset.

```bash
docker compose -f ../../../L1-host-stack-kits/lepton/infra/mailpit/docker-compose.yml up -d
```

Delivery is synchronous. No Boson runtime is installed, so a code is readable from
Mailpit or the sink as soon as the action returns.

## Run

```bash
export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR=target-lepton-uf-app
# From the lepton-uf-app workspace root. Builds SSR + hydrate (wasm-split for
# Lazy /user routes), then Playwright.
cargo leptos end-to-end --split --project lepton-uf-app-e2e
```

Do not interrupt the end-to-end run. It stops on its own when Playwright finishes.

To watch the browser, serve in one terminal and drive Playwright in another:

```bash
cargo leptos watch --split --project lepton-uf-app-e2e
# other terminal (WSLg or a display is required on WSL):
cd lepton-uf-app-e2e/end2end
npm ci && npx playwright install chromium
npm run test:headed
```

## Seeding

`POST /api/test/seed-data` is mounted for the harness only. Never mount it on a
production host. Scenarios and builders live in
[`lepton-test-support`](../../../L1-host-stack-kits/lepton/lepton-test-support/):
`auth_basic_user`, `auth_unverified_user`, `auth_confirm_*`, `auth_reset_token`,
`auth_user_with_totp`, and siblings.

Shared Playwright helpers live in [`end2end/shared/`](end2end/shared/) (`seedTestData`,
`signInAs`, `signupNewUser`, Mailpit and SMS readers). Specs import from
`end2end/tests/fixtures.ts`, which re-exports that folder.

## Product Playwright catalog

| Spec | Scenario IDs |
|------|----------------|
| `profile.spec.ts` | `pw-profile-update-happy`, `pw-profile-empty-name-sad`, `pw-profile-anon-redirect-sad` |
| `profile_photo.spec.ts` | `pw-profile-photo-upload-happy`, `pw-profile-photo-reject-sad` |
| `appearance.spec.ts` | `pw-appearance-save-happy`, `pw-appearance-anon-redirect-sad` |
| `account_email_password.spec.ts` | `pw-account-change-password-happy`, `pw-account-change-password-bad-current-sad`, `pw-account-change-email-happy`, `pw-account-change-email-invalid-sad` |
| `account_settings_composition.spec.ts` | `pw-account-settings-composition-happy` |
| `shell_menu.spec.ts` | `pw-shell-anon-signin-opens-dialog-happy`, `pw-shell-signed-in-nav-profile-happy`, `pw-shell-signed-out-profile-gated-sad` |
| `auth_gate_modal.spec.ts` | `pw-auth-gate-modal-signin-opens-happy`, `pw-auth-gate-modal-signup-opens-happy`, `pw-auth-gate-modal-signin-unlock-happy`, `pw-auth-gate-modal-bad-creds-sad`, `pw-auth-gate-take-me-back-sad` |
| `auth_referer.spec.ts` | `pw-auth-referer-safe-happy`, `pw-auth-referer-gate-path-happy`, `pw-auth-referer-evil-sad` |
| `account_wipe_totp.spec.ts` | `pw-account-wipe-totp-happy`, `pw-account-wipe-totp-bad-code-sad` |
| `totp_enroll.spec.ts` | `pw-totp-enroll-happy`, `pw-totp-enroll-bad-code-sad` |
| `oauth_link_settings.spec.ts` | `pw-oauth-link-google-happy`, `pw-oauth-link-account-taken-sad` |
| `delivery_settings.spec.ts` | `pw-lepton-delivery-settings-happy`, `pw-lepton-delivery-settings-denied-sad`, `pw-lepton-delivery-settings-test-send-failed-sad`, `pw-lepton-delivery-settings-stepup-sad` |
| `service-users.spec.ts` | `pw-lepton-service-users-list-empty-happy`, `pw-lepton-service-users-no-permission-sad`, `pw-lepton-service-users-anonymous-sad`, `pw-lepton-service-users-create-full-happy`, `pw-lepton-service-users-reveal-once-happy`, `pw-lepton-service-users-handle-clash-sad`, `pw-lepton-service-users-handle-invalid-sad`, `pw-lepton-service-users-profile-empty-name-sad`, `pw-lepton-service-users-access-not-grantable-sad`, `pw-lepton-service-users-quota-sad`, `pw-lepton-service-users-token-limit-sad`, `pw-lepton-service-users-foreign-service-user-sad`, `pw-lepton-service-users-suspended-service-user-sad`, `pw-lepton-service-users-dark-mode`, `pw-lepton-service-users-mobile-390`, `pw-lepton-service-users-keyboard-create` |

## Sign-in and delivery

The page at `/user/delivery` runs on the same boot a cell uses. The host seeds its
delivery settings from env once (Mailpit for mail, the mocks below for SendGrid and
Twilio Verify, the mock IdP for OAuth), keeps the secrets in Neutrino, and applies a
saved change without a restart. Writes ask for a fresh authenticator code, so a spec
that saves twice waits for the next 30 second step.

Lab-only routes, never mounted on a production host:

| Route | Role |
|-------|------|
| `POST /api/test/delivery-admin` | Adds a user (by email) to `lepton.delivery.admins`, standing in for an operator on a cell |
| `POST /mock/sendgrid/v3/mail/send` | SendGrid stand-in that records the `Authorization` header and recipient |
| `POST /mock/twilio/v2/Services/{sid}/Verifications` | Twilio Verify stand-in; also forwards the code to the SMS sink so OTP reads keep working |
| `GET /api/test/mock-providers` | What the mocks received |
| `POST /api/test/mock-providers/reset` and `.../fail/{provider}/{status}` | Clear the record; make a provider answer with a status |

## Service users

The Service users pages (`/user/service-users`) run against Gauge's service user gate, a "Spectra readers"
group with no owners, and an in-memory token store in `src/service_users.rs`. The token
store hands out random `labtk_` secrets and counts them per service user; it stands in
for a host's real token service, which a host covers in its own tests. Group
grants go through Gauge for real, so only a super user can put a service user in Spectra
readers here, and that asks for an authenticator code.

Accessibility checks run axe (`@axe-core/playwright`, WCAG 2 A and AA) on the
list, each create step and the token reveal, in light and dark mode. Dark mode
is set through the Appearance page, because Orbital doesn't follow the
browser's color scheme.

Lab-only route, never mounted on a production host:

| Route | Role |
|-------|------|
| `POST /api/test/service-users` | `join_creators` and `super_user` (by email), `seed_service_users` (create and optionally pause service users for a user), `seed_tokens` (pretend a service user has N tokens), `in_readers` (is a service user in Spectra readers) |

`pw-account-change-email-happy` requires Mailpit and asserts “Verification sent…” plus a message to the new address.

Profile photos use `POST/GET /api/files/*` on this host (`LocalDiskBlobStore` under `uploads/`).
Client `profile_id` on upload is a hint for the harness; production hosts must authorize
from the session (see workspace README Security → Host duties).

## Deferred to the kit (do not re-prove here)

Primary coverage for these stays in
[`lepton-auth-ui-e2e/end2end/tests/`](../../../L1-host-stack-kits/lepton/lepton-auth-ui-e2e/end2end/tests/):

| Concern | Kit scenario / spec |
|---------|---------------------|
| Confirm funnel (email, phone, banner, prompt resume) | `confirm_account.spec.ts` |
| MFA matrix at sign-in (TOTP, recovery, WebAuthn skip and reject) | `signin_mfa.spec.ts` |
| Passkey enroll, revoke, trusted browser | `devices.spec.ts` |
| OAuth sign-in and callback against the mock IdP | `oauth.spec.ts` |
| Step-up re-auth modal (password and TOTP ladders) | `step_up_modal.spec.ts` |
| Password-only account wipe | `account_wipe.spec.ts` (`pw-account-wipe-*`) |
| Route pages, dialog, logout, reset request and confirm | `auth.spec.ts` |
| Authenticator disable / regenerate recovery | `totp_enroll.spec.ts` (kit rows beyond product enroll) |

The `lepton-shell` integration tests `product_surface` and `shell_step_up_surface`
read source and assert that testids and server actions are still wired. They catch a
deleted testid, not a broken flow: they never render a page, run a server function,
or click anything. Treat them as composition smokes, not a substitute for the
Playwright rows above.

## Assets

`public/orbital-theme-baseline.css` is copied from Orbital's generated first-paint
baseline. Regenerate it from the `orbital` crate build when `LEPTOS_BASE_PATH` or the
theme tokens change.
