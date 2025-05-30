# fishhook


## fishhook-rs

A Rust port of [fishhook](https://github.com/facebook/fishhook) — a library that enables dynamically rebinding symbols
in Mach-O binaries at runtime. Useful for intercepting system functions like `malloc`, `free`, or `open` on Apple
platforms.

> **Platform support**: Currently tested only on macOS (aarch64-apple-darwin)

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
fishhook = "0.1"
```

### Usage
Example below uses [ctor](https://github.com/mmastrac/rust-ctor) for invoking ***init()*** first
```rust
use fishhook::{register, Rebinding};

#[ctor::ctor]
fn init() {
    unsafe {
        register(vec![
            Rebinding {
                name: "malloc".to_string(),
                function: my_malloc as *const c_void,
            },
            Rebinding {
                name: "calloc".to_string(),
                function: my_calloc as *const c_void,
            },
            Rebinding {
                name: "realloc".to_string(),
                function: my_realloc as *const c_void,
            },
            Rebinding {
                name: "free".to_string(),
                function: my_free as *const c_void,
            },
            Rebinding {
                name: "atexit".to_string(),
                function: my_exit as *const c_void,
            },
        ]);
    }
}
```

License: MIT
