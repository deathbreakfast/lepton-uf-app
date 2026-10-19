//! Playwright host for the `lepton-uf-app` product crates.
#![allow(missing_docs)]
#![recursion_limit = "256"]

pub mod app;

#[cfg(feature = "ssr")]
pub mod boot;
#[cfg(feature = "ssr")]
pub mod mock_providers;
#[cfg(feature = "ssr")]
pub mod permissions;
#[cfg(feature = "ssr")]
pub mod seed;
#[cfg(feature = "ssr")]
pub mod service_users;

pub use app::{shell, App};

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use app::App;

    #[cfg(feature = "hydrate")]
    {
        std::panic::set_hook(Box::new(|info| {
            uf_product::hide_boot_loader();
            console_error_panic_hook::hook(info);
        }));
        // Lazy `/user/*` routes need hydrate_lazy (`cargo leptos --split`).
        // hydrate_body panics: "lazy routes not supported with hydrate_body()".
        leptos::mount::hydrate_lazy(App);
        uf_product::hide_boot_loader();
    }
}
