#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

use super::Rebinding;
use core::ffi::{c_char, c_int, c_void};
use elf::abi::{DT_NULL, DT_RELA, DT_RELASZ, DT_STRSZ, DT_STRTAB, DT_SYMTAB};
use elf::dynamic::Elf64_Dyn;
use elf::relocation::Elf64_Rela;
use libc::{
    dl_iterate_phdr, dl_phdr_info, mprotect, sysconf, Elf64_Phdr, Elf64_Sym, PROT_READ, PROT_WRITE,
    PT_DYNAMIC, _SC_PAGESIZE,
};
use std::ffi::CStr;
use std::ptr;
use std::sync::Mutex;

const R_X86_64_JUMP_SLOT: u32 = 7;
const R_X86_64_GLOB_DAT: u32 = 6;

#[inline(always)]
fn rela_sym(r_info: u64) -> u32 {
    (r_info >> 32) as u32
}
#[inline(always)]
fn rela_type(r_info: u64) -> u32 {
    (r_info & 0xffffffff) as u32
}

static BINDINGS: Mutex<Vec<Rebinding>> = Mutex::new(Vec::new());

pub unsafe fn register(bindings: Vec<Rebinding>) {
    {
        let mut g = BINDINGS.lock().unwrap();
        *g = bindings;
    }
    unsafe { rebind_all_loaded_images() };
}

#[no_mangle]
pub unsafe extern "C" fn rebind_all_loaded_images() {
    unsafe {
        dl_iterate_phdr(Some(iter_cb), ptr::null_mut());
    }
}

unsafe extern "C" fn iter_cb(info: *mut dl_phdr_info, _size: usize, _data: *mut c_void) -> c_int {
    let info = &*info;
    let base = info.dlpi_addr as usize;
    let phnum = info.dlpi_phnum as usize;
    let phdrs = std::slice::from_raw_parts(info.dlpi_phdr as *const Elf64_Phdr, phnum);

    let mut dynamic: *const Elf64_Dyn = ptr::null();
    for ph in phdrs {
        if ph.p_type == PT_DYNAMIC {
            dynamic = (base + ph.p_vaddr as usize) as *const Elf64_Dyn;
            break;
        }
    }
    if dynamic.is_null() {
        return 0;
    }

    let mut strtab = 0usize;
    let mut strsz = 0usize;
    let mut symtab = 0usize;

    let mut rela = 0usize;
    let mut relasz = 0usize;

    let mut d = dynamic;
    loop {
        let tag = (*d).d_tag;
        if tag == DT_NULL {
            break;
        }
        match tag {
            DT_STRTAB => strtab = (*d).d_un as usize,
            DT_STRSZ => strsz = (*d).d_un as usize,
            DT_SYMTAB => symtab = (*d).d_un as usize,
            DT_RELA => rela = (*d).d_un as usize,
            DT_RELASZ => relasz = (*d).d_un as usize,
            _ => {}
        }
        d = d.add(1);
    }

    if strtab == 0 || symtab == 0 || strsz == 0 {
        return 0;
    }

    let strtab_ptr = strtab as *const u8;
    let symtab_ptr = symtab as *const Elf64_Sym;

    // Patch non-PLT relocations (.rela.dyn)
    if rela != 0 && relasz != 0 {
        let relas = rela as *const Elf64_Rela;
        let count = relasz / core::mem::size_of::<Elf64_Rela>();
        unsafe { patch_relas(base, relas, count, symtab_ptr, strtab_ptr, strsz, false) };
    }

    0
}

unsafe fn patch_relas(
    base: usize,
    relas: *const Elf64_Rela,
    count: usize,
    symtab: *const Elf64_Sym,
    strtab: *const u8,
    strsz: usize,
    is_plt: bool,
) {
    let bindings = BINDINGS.lock().unwrap();
    if bindings.is_empty() {
        return;
    }

    for i in 0..count {
        let r = &*relas.add(i);
        let rtype = rela_type(r.r_info);

        // Only patch relevant relocation types
        if is_plt {
            if rtype != R_X86_64_JUMP_SLOT {
                continue;
            }
        } else {
            // Common non-PLT function/data imports:
            if rtype != R_X86_64_GLOB_DAT && rtype != R_X86_64_JUMP_SLOT {
                continue;
            }
        }

        let sym_idx = rela_sym(r.r_info) as usize;
        let sym = &*symtab.add(sym_idx);

        let name_off = sym.st_name as usize;
        if name_off >= strsz {
            continue;
        }

        let name_ptr = strtab.add(name_off) as *const c_char;
        let Ok(sym_name) = CStr::from_ptr(name_ptr).to_str() else {
            continue;
        };
        if sym_name.is_empty() {
            continue;
        }

        // r_offset is a VA inside the object; add base to get runtime address.
        let slot = (base + r.r_offset as usize) as *mut *const c_void;

        for b in bindings.iter() {
            if b.name.is_empty() {
                continue;
            }
            let Ok(wanted) = CStr::from_ptr(b.name.as_ptr() as *const i8).to_str() else {
                continue;
            };
            if wanted != sym_name {
                continue;
            }

            if *slot == b.function as *const c_void {
                break;
            }

            unsafe { make_writable_and_patch(slot, b.function as *const c_void) };
            break;
        }
    }
}

unsafe fn make_writable_and_patch(slot: *mut *const c_void, new: *const c_void) {
    let page = sysconf(_SC_PAGESIZE) as usize;
    let addr = slot as usize;
    let page_start = addr & !(page - 1);

    let rc = mprotect(page_start as *mut c_void, page, PROT_READ | PROT_WRITE);
    if rc != 0 {
        return;
    }

    *slot = new;

    let _ = mprotect(page_start as *mut c_void, page, PROT_READ);
}
