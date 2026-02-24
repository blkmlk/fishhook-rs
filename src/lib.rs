//!
//!# fishhook-rs
//!
//! A Rust port of [fishhook](https://github.com/facebook/fishhook) — a library that enables dynamically rebinding symbols
//! for Linux and Mach-O binaries at runtime. Useful for intercepting system functions like `malloc`, `free`, or `open`.
//!
//! > **Platform support**: Currently tested on Linux (x86_64) and macOS (aarch64-apple-darwin)
//!
//! ## Installation
//!
//! Add to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! fishhook = "0.2"
//! ```
//!
//! ## Usage
//! Example below uses [ctor](https://github.com/mmastrac/rust-ctor) for invoking ***init()*** first
//! ```rust
//! use fishhook::{register, Rebinding};
//!
//! #[ctor::ctor]
//! fn init() {
//!     unsafe {
//!         register(vec![
//!            Rebinding {
//!                name: "malloc".to_string(),
//!                function: my_malloc as *const () as usize,
//!            },
//!            Rebinding {
//!                name: "calloc".to_string(),
//!                function: my_calloc as *const () as usize,
//!            },
//!            Rebinding {
//!                name: "realloc".to_string(),
//!                function: my_realloc as *const () as usize,
//!            },
//!            Rebinding {
//!                name: "free".to_string(),
//!                function: my_free as *const () as usize,
//!            },
//!         ]);
//!     }
//! }
//! ```

mod arch;
pub use arch::Rebinding;

pub unsafe fn register(bindings: Vec<arch::Rebinding>) {
    arch::register_bindings(bindings);
}
