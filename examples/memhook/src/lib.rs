mod backtrace;
mod collector;
mod tree;

use crate::collector::Collector;
use fishhook::{register, Rebinding};
use libc::{dlsym, size_t, RTLD_NEXT};
use std::ffi::c_void;
use std::sync::Once;

static INIT: Once = Once::new();
static mut ORIGINAL_MALLOC: Option<unsafe extern "C" fn(size: size_t) -> *mut c_void> = None;
static mut ORIGINAL_CALLOC: Option<unsafe extern "C" fn(num: size_t, size: size_t) -> *mut c_void> =
    None;
static mut ORIGINAL_REALLOC: Option<
    unsafe extern "C" fn(ptr: *mut c_void, size: size_t) -> *mut c_void,
> = None;
static mut ORIGINAL_FREE: Option<unsafe extern "C" fn(ptr: *mut c_void)> = None;
static mut COLLECTOR: Option<Collector> = None;

#[no_mangle]
pub unsafe extern "C" fn my_malloc(size: size_t) -> *mut c_void {
    let original_malloc = ORIGINAL_MALLOC.unwrap();
    let ptr = original_malloc(size);

    COLLECTOR.as_mut().unwrap().on_malloc(size, ptr as usize);

    ptr
}

#[no_mangle]
pub unsafe extern "C" fn my_free(ptr: *mut c_void) {
    let original_free = ORIGINAL_FREE.unwrap();
    original_free(ptr);

    COLLECTOR.as_mut().unwrap().on_free(ptr as usize);
}

unsafe fn prepare() {
    INIT.call_once(|| {
        COLLECTOR = Some(Collector::new("/tmp/flamegraph.svg".to_string()));

        let symbol = b"malloc\0";
        let malloc_ptr = dlsym(RTLD_NEXT, symbol.as_ptr() as *const _);
        if !malloc_ptr.is_null() {
            ORIGINAL_MALLOC = Some(std::mem::transmute(malloc_ptr));
        } else {
            eprintln!("Error: Could not locate original malloc!");
        }

        let symbol = b"free\0";
        let free_ptr = dlsym(RTLD_NEXT, symbol.as_ptr() as *const _);
        if !free_ptr.is_null() {
            ORIGINAL_FREE = Some(std::mem::transmute(free_ptr));
        } else {
            eprintln!("Error: Could not locate original free!");
        }
    });
}

#[ctor::ctor]
fn init() {
    unsafe {
        prepare();

        register(vec![
            Rebinding {
                name: "malloc".to_string(),
                function: my_malloc as *const c_void,
            },
            Rebinding {
                name: "free".to_string(),
                function: my_free as *const c_void,
            },
        ]);
    }

    println!("Initializing memory system...");
}

#[no_mangle]
pub extern "C" fn deinit_library() {
    unsafe {
        println!("Library deinitialized.");
    }
}
