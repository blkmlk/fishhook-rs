// lib.rs

use libc::{_dyld_image_count, bind, c_char, c_void, RTLD_DEFAULT, RTLD_NEXT};
use std::ffi::CString;
use std::ptr;
use mach2::kern_return::KERN_SUCCESS;
use mach2::message::mach_msg_type_number_t;
use mach2::traps::mach_task_self;
use mach2::vm::{mach_vm_protect, mach_vm_write};
use mach2::vm_types::{mach_vm_address_t, mach_vm_size_t};

pub struct Rebinding {
    replacement: *const c_void,     // pointer to the replacement function
    original: *mut *const c_void,   // pointer to store the original function
}

impl Rebinding {
    /// Create a new Rebinding entry
    pub fn new(replacement: *const c_void, original: *mut *const c_void) -> Self {
        Self {
            replacement,
            original,
        }
    }
}

/// Finds and replaces symbols at runtime.
pub unsafe fn rebind_symbols(bindings: &[Rebinding]) -> Result<(), &'static str> {
    for binding in bindings {
        // Store the original function pointer if requested
        if binding.original.is_null() {
            return Err("Original function pointer is null.");
        }

        // Ensure replacement function is valid
        if binding.replacement.is_null() {
            return Err("Replacement function pointer is null.");
        }

        // Change memory protection to allow writing
        let result = mach_vm_protect(
            mach_task_self(),
            binding.original as mach_vm_address_t,
            std::mem::size_of::<*const c_void>() as mach_vm_address_t,
            0,
            mach2::vm_prot::VM_PROT_WRITE | mach2::vm_prot::VM_PROT_READ,
        );

        if result != KERN_SUCCESS {
            println!("result: {}", result);
            return Err("Failed to change memory protection to writable.");
        }

        // Write the replacement function pointer to the symbol location
        let write_result = mach_vm_write(
            mach_task_self(),
            binding.original as mach_vm_size_t,
            binding.replacement as usize,
            std::mem::size_of::<*const c_void>() as mach_msg_type_number_t,
        );

        if write_result != KERN_SUCCESS {
            return Err("Failed to write replacement function pointer.");
        }

        // Restore original memory protection to read/execute
        let protect_result = mach_vm_protect(
            mach_task_self(),
            binding.original as mach_vm_address_t,
            std::mem::size_of::<*const c_void>() as mach_vm_size_t,
            0,
            mach2::vm_prot::VM_PROT_READ | mach2::vm_prot::VM_PROT_EXECUTE,
        );

        if protect_result != KERN_SUCCESS {
            return Err("Failed to restore memory protection to read/execute.");
        }
    }

    Ok(())
}

// Load a symbol by name from the current process
extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

// Example replacement function for demonstration
extern "C" fn my_puts(s: *const c_char) -> i32 {
    println!("Hooked puts: {:?}", unsafe { CString::from_raw(s as *mut c_char) });
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use libc::puts;

    #[test]
    fn test_symbol_rebinding() {
        unsafe {
            // Prepare the original pointer storage
            let mut original_puts: *const c_void = ptr::null();

            // Create a rebinding for `puts` function
            let rebinding = Rebinding::new("puts", my_puts as *const c_void, &mut original_puts);
            let result = rebind_symbols(&[rebinding]);

            // Check if the rebinding was successful
            assert!(result.is_ok());

            // Call puts to test if it has been hooked
            println!("Calling puts after rebinding...");
            puts(b"Hello, World!\0".as_ptr() as *const c_char);

            // You may add more assertions to verify expected behavior if feasible
        }
    }
}