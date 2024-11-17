use fishhook::{register, Rebinding};
use libc::size_t;
use std::ffi::c_void;

static mut ORIGINAL_MALLOC: Option<unsafe extern "C" fn(size: size_t) -> *mut c_void> = None;
static mut ORIGINAL_CALLOC: Option<unsafe extern "C" fn(num: size_t, size: size_t) -> *mut c_void> =
    None;
static mut ORIGINAL_REALLOC: Option<
    unsafe extern "C" fn(ptr: *mut c_void, size: size_t) -> *mut c_void,
> = None;
static mut ORIGINAL_FREE: Option<unsafe extern "C" fn(ptr: *mut c_void)> = None;

#[no_mangle]
pub unsafe extern "C" fn my_malloc(size: size_t) -> *mut c_void {
    let original_malloc = ORIGINAL_MALLOC.unwrap();
    let ptr = original_malloc(size);

    println!("MY malloc({}) = {:?}", size, ptr);

    ptr
}

#[ctor::ctor]
fn init() {
    unsafe {
        register(&[Rebinding {
            name: "malloc".to_string(),
            replacement: my_malloc as *const c_void,
            replaced: core::ptr::null(),
        }]);
    }

    println!("Initializing memory system...");
}
