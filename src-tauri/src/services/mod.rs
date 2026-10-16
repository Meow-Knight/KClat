//! The owners of the app's state (docs/ARCHITECTURE.md §5.4).

pub mod cat_service;
pub mod hooks_service;
pub mod settings_service;

use tokio::sync::watch;

/// Runs `f` on the async runtime with each value sent on `rx` from now on, until the sender is gone. `f` gets a
/// clone: the channel is not borrowed while it runs, so it may call back into the service.
pub fn on_each<T>(mut rx: watch::Receiver<T>, mut f: impl FnMut(T) + Send + 'static)
where
    T: Clone + Send + Sync + 'static,
{
    tauri::async_runtime::spawn(async move {
        while rx.changed().await.is_ok() {
            let value = rx.borrow_and_update().clone();
            f(value);
        }
    });
}
