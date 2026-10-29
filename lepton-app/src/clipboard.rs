//! Copy text to the system clipboard from a button.

/// Write `text` to the clipboard. Does nothing on the server.
pub fn copy_text(text: &str) {
    #[cfg(feature = "hydrate")]
    {
        if let Some(window) = web_sys::window() {
            let _ = window.navigator().clipboard().write_text(text);
        }
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = text;
    }
}

/// Write `text` to the clipboard and call `done(true)` once the browser accepts it, or
/// `done(false)` when it refuses (no permission, no secure context). On the server it
/// calls `done(false)`.
pub fn copy_text_checked(text: &str, done: impl FnOnce(bool) + 'static) {
    #[cfg(feature = "hydrate")]
    {
        let Some(window) = web_sys::window() else {
            done(false);
            return;
        };
        let promise = window.navigator().clipboard().write_text(text);
        leptos::task::spawn_local_scoped(async move {
            let ok = wasm_bindgen_futures::JsFuture::from(promise).await.is_ok();
            done(ok);
        });
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = text;
        done(false);
    }
}
