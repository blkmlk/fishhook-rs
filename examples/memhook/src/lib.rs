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

#[no_mangle]
pub unsafe extern "C" fn my_malloc(size: size_t) -> *mut c_void {
    let original_malloc = ORIGINAL_MALLOC.unwrap();
    let ptr = original_malloc(size);

    println!("MY malloc({}) = {:?}", size, ptr);

    ptr
}

unsafe fn preserve() {
    INIT.call_once(|| {
        let symbol = b"malloc\0";
        let malloc_ptr = dlsym(RTLD_NEXT, symbol.as_ptr() as *const _);
        if !malloc_ptr.is_null() {
            ORIGINAL_MALLOC = Some(std::mem::transmute(malloc_ptr));
        } else {
            eprintln!("Error: Could not locate original malloc!");
        }
    });
}

#[ctor::ctor]
fn init() {
    unsafe {
        preserve();

        register(&[Rebinding {
            name: "malloc".to_string(),
            function: my_malloc as *const c_void,
        }]);
    }

    println!("Initializing memory system...");
}
