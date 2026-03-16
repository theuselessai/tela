pub mod clipboard;
pub mod console;
pub mod env;
pub mod fetch;
pub mod filesystem;
pub mod storage;
pub mod timers;
pub mod websocket;

pub use clipboard::register_clipboard;
pub use console::register_console;
pub use env::register_env;
pub use fetch::register_fetch;
pub use filesystem::register_filesystem;
pub use storage::register_storage;
pub use timers::{register_timers, TimerHandles};
pub use websocket::{register_websocket, WsSenders};
