use std::ffi::c_void;
use std::panic::resume_unwind;
use libc::{res_init, size_t};
use fishhook::rebind_symbols;

static mut ORIGINAL_MALLOC: Option<unsafe extern "C" fn(size: size_t) -> *mut c_void> = None;
static mut ORIGINAL_CALLOC: Option<unsafe extern "C" fn(num: size_t, size: size_t) -> *mut c_void> = None;
static mut ORIGINAL_REALLOC: Option<unsafe extern "C" fn(ptr: *mut c_void, size: size_t) -> *mut c_void> = None;
static mut ORIGINAL_FREE: Option<unsafe extern "C" fn(ptr: *mut c_void)> = None;

// fishho
// if ORIGINAL_MALLOC.is_none() {
// ORIGINAL_MALLOC = Some());
// }
// let original_malloc = ORIGINAL_MALLOC.unwrap();
// let ptr = original_malloc(size);


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
        if ORIGINAL_MALLOC.is_none() {
            ORIGINAL_MALLOC = Some(std::mem::transmute(libc::dlsym(libc::RTLD_NEXT, "malloc\0".as_ptr().cast())));
        }

        let rebinding = fishhook::Rebinding::new(my_malloc as *const c_void, ORIGINAL_MALLOC.unwrap() as *mut *const c_void);

        let result = rebind_symbols(&[rebinding]);
        result.unwrap();
    }

    println!("Initializing memory system...");
}