#[cfg_attr(target_os = "macos", path = "darwin.rs")]
#[cfg_attr(target_os = "linux", path = "linux.rs")]
mod platform;

pub use platform::*;

#[derive(Clone)]
pub struct Rebinding {
    pub name: String,
    pub function: usize,
}
