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
