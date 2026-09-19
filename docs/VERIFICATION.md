# lepton-uf-app verification

Workspace members: `lepton-shell`, `lepton-app`, `lepton-auth-app`,
`lepton-uf-app-e2e`, `examples/lepton-mount-host`.

## PR CI parity (run before push / opening a PR)

Matches [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) required jobs.
Toolchain: **nightly**. Global env includes `RUSTFLAGS=-D warnings`.

| CI job | Local command |
|--------|----------------|
| fmt | `cargo fmt -p lepton-shell -p lepton-app -p lepton-auth-app -p lepton-mount-host -p lepton-uf-app-e2e -- --check` |
| clippy | product crates + `lepton-uf-app-e2e` + `lepton-mount-host` (see Gates) |
| test | check/run/test block in Gates |
| docs | `RUSTDOCFLAGS="-D warnings" cargo doc -p lepton-shell -p lepton-app -p lepton-auth-app --features ssr --no-deps` |
| leptos-lints | dylint hydrate commands below (`nightly-2025-05-14`) |
| e2e | Mailpit + `cargo leptos end-to-end --split --project lepton-uf-app-e2e` |

## Environment

```bash
export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR=target-lepton-uf-app
export RUSTFLAGS="-D warnings"
```

This workspace pins `rust-toolchain.toml` to `nightly` (Leptos `nightly` features + Orbital). Use that channel for the commands below.

## Gates

```bash
cargo fmt -p lepton-shell -p lepton-app -p lepton-auth-app -p lepton-mount-host -p lepton-uf-app-e2e -- --check
cargo clippy -p lepton-shell -p lepton-app -p lepton-auth-app --features ssr --all-targets -- -D warnings
cargo clippy -p lepton-uf-app-e2e --features ssr --all-targets -- -D warnings
cargo clippy -p lepton-mount-host --all-targets -- -D warnings
cargo check -p lepton-shell -p lepton-app -p lepton-auth-app --features ssr
cargo check -p lepton-mount-host
cargo check -p lepton-uf-app-e2e --features ssr
cargo check -p lepton-app --target wasm32-unknown-unknown --features hydrate
cargo check -p lepton-auth-app --target wasm32-unknown-unknown --features hydrate
cargo run -p lepton-mount-host
cargo test -p lepton-app --features ssr --lib -- validate_display_name
cargo test -p lepton-shell --features ssr --test workspace_members
cargo test -p lepton-shell --features ssr --test shell_step_up_surface
cargo test -p lepton-shell --features ssr --test product_surface
RUSTDOCFLAGS="-D warnings" cargo doc -p lepton-shell -p lepton-app -p lepton-auth-app --features ssr --no-deps
```

Teaching host: [`examples/lepton-mount-host`](../examples/lepton-mount-host/) —
path/auth/inventory **contract smoke** (no Leptos UI). Success line:
`lepton_mount_host: OK — /user protect + /auth public + inventory`.
Real Leptos mount of the three crates: [`lepton-uf-app-e2e`](../lepton-uf-app-e2e/).
Workspace `missing_docs` is deny. `lepton-app` / `lepton-auth-app` allow macro-emitted
undocumented associated items (`#![allow(missing_docs)]` with reason). Rustdoc deny
gate: `lepton-shell`, `lepton-app`, and `lepton-auth-app` under
`RUSTDOCFLAGS="-D warnings"`.

### Product Playwright (`lepton-uf-app-e2e`)

Needs Mailpit on SMTP `1025` / API `8025` (change-email assertions). The product
host uses `Lazy` `/user` routes, so end-to-end must pass `--split`:

```bash
docker compose -f ../../L1-host-stack-kits/lepton/infra/mailpit/docker-compose.yml up -d
# From the lepton-uf-app workspace root. Builds SSR + hydrate (wasm-split), then Playwright.
cargo leptos end-to-end --split --project lepton-uf-app-e2e
```

Do not interrupt the end-to-end run; it exits when Playwright finishes.

Kit-owned domain matrices (confirm funnel, MFA at sign-in, passkeys, OAuth
sign-in/callback, step-up modal, password-only wipe) stay in `lepton-auth-ui-e2e`.
Product host covers profile photo `/api/files`, TOTP enroll, and OAuth link.
See [`lepton-uf-app-e2e/README.md`](../lepton-uf-app-e2e/README.md) for the catalog.

### leptos-lints (CI job `leptos-lints`)

Needs `cargo-dylint` / `dylint-link` 6.0.1 and toolchain `nightly-2025-05-14`
(see `.github/workflows/ci.yml`). Hydrate UI crates (`--no-deps`):

```bash
# cargo install cargo-dylint --locked --version 6.0.1
# cargo install dylint-link --locked --version 6.0.1
# rustup toolchain install nightly-2025-05-14 --component rustc-dev,llvm-tools-preview
export CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback
export RUSTFLAGS="-D warnings -Zcrate-attr=feature(stdarch_x86_avx512)"
cargo dylint --all -p lepton-app --no-deps -- --features hydrate
cargo dylint --all -p lepton-auth-app --no-deps -- --features hydrate
cargo dylint --all -p lepton-shell --no-deps -- --features hydrate
```
