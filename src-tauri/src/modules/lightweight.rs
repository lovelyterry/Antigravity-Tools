// Headless standalone lightweight module dummy
// In web-server mode, there is no desktop window or WebView to minimize/destroy.

pub fn ensure_main_window<T>(_app: &T) -> Result<(), String> {
    Ok(())
}
