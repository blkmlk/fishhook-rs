extern crate libc;

use crate::arch::MachHeaderT;
use goblin::mach::constants::{
    SECTION_TYPE, SEG_DATA, SEG_LINKEDIT, S_LAZY_SYMBOL_POINTERS, S_NON_LAZY_SYMBOL_POINTERS,
};
use goblin::mach::load_command::{LC_DYSYMTAB, LC_SYMTAB};
use libc::{c_char, c_int, c_void, dladdr, uintptr_t, Dl_info};
use mach2::kern_return::KERN_SUCCESS;
use mach2::traps::mach_task_self;
use mach2::vm::mach_vm_protect;
use mach2::vm_prot::{VM_PROT_COPY, VM_PROT_READ, VM_PROT_WRITE};
use mach2::vm_types::{mach_vm_address_t, mach_vm_size_t};
use object::macho::INDIRECT_SYMBOL_ABS;
use object::macho::INDIRECT_SYMBOL_LOCAL;
use object::{Object, ObjectSymbolTable};
use std::ffi::CStr;
use std::ptr::{null, null_mut};

static mut BINDINGS: Vec<Rebinding> = Vec::new();

const SEG_DATA_CONST: &str = "__DATA_CONST";

#[cfg(target_pointer_width = "64")]
mod arch {
    use goblin::mach::load_command::{Section64, SegmentCommand64};
    use goblin::mach::symbols::Nlist64;
    use libc::mach_header_64;

    pub const LC_SEGMENT_ARCH_DEPENDENT: u32 = libc::LC_SEGMENT_64;
    pub type NlistT = Nlist64;
    pub type SectionT = Section64;
    pub type SegmentCommandT = SegmentCommand64;
    pub type MachHeaderT = mach_header_64;
}

#[cfg(target_pointer_width = "32")]
mod arch {
    use goblin::mach::load_command::{Section32, SegmentCommand32};
    use goblin::mach::symbols::Nlist32;
    use libc::mach_header;
    use object::macho::Section64;

    pub const LC_SEGMENT_ARCH: u32 = libc::LC_SEGMENT;
    pub type NlistT = Nlist32;
    pub type SectionT = Section32;
    pub type SegmentCommandT = SegmentCommand32;
    pub type MachHeaderT = mach_header;
}

extern "C" {
    fn _dyld_register_func_for_add_image(callback: extern "C" fn(*const c_void, c_int));
}

#[derive(Clone)]
pub struct Rebinding {
    pub name: String,
    pub replacement: *const c_void,
    pub replaced: *const c_void,
}

pub unsafe fn register(bindings: &[Rebinding]) {
    BINDINGS = bindings.to_vec();

    _dyld_register_func_for_add_image(add_image);
}

extern "C" fn add_image(header: *const c_void, slide: c_int) {
    // println!(
    //     "Adding image: Header: {:#x} {}",
    //     header as uintptr_t,
    //     size_of::<arch::MachHeaderT>()
    // );
    unsafe { rebind_for_image(header as *const c_void, slide) }
}

unsafe fn rebind_for_image(header: *const c_void, slide: c_int) {
    let mut dl_info = new_dl_info();
    if dladdr(header, &mut dl_info as *mut Dl_info) == 0 {
        return;
    };

    let name = CStr::from_ptr(dl_info.dli_fname)
        .to_string_lossy()
        .to_string();

    println!("INFO: {}", name);

    let mut linked_segment = None;
    let mut symtab_cmd = None;
    let mut dysymtab = None;

    let header = header as *const MachHeaderT;

    let mut cur_seg_cmd =
        header.byte_add(size_of::<MachHeaderT>() as uintptr_t) as *const arch::SegmentCommandT;

    for _ in 0..(*header).ncmds {
        match (*cur_seg_cmd).cmd {
            arch::LC_SEGMENT_ARCH_DEPENDENT => {
                if eq_u8((*cur_seg_cmd).segname, SEG_LINKEDIT) {
                    linked_segment = Some(cur_seg_cmd);
                }
            }
            LC_SYMTAB => {
                symtab_cmd = Some(cur_seg_cmd as *const goblin::mach::load_command::SymtabCommand);
            }
            LC_DYSYMTAB => {
                dysymtab = Some(cur_seg_cmd as *const goblin::mach::load_command::DysymtabCommand);
            }
            _ => {}
        }

        cur_seg_cmd = cur_seg_cmd.byte_add((*cur_seg_cmd).cmdsize as uintptr_t);
    }
    println!("Linked segment: {:?}", linked_segment);
    println!("SYMTAB: {:?}", symtab_cmd);
    println!("DYSYMTAB: {:?}", dysymtab);

    let (Some(linked_segment), Some(symtab_cmd), Some(dysymtab)) =
        (linked_segment, symtab_cmd, dysymtab)
    else {
        return;
    };

    if (*dysymtab).nindirectsyms == 0 {
        return;
    }

    let linkedit_base = slide as uintptr_t + (*linked_segment).vmaddr as uintptr_t
        - (*linked_segment).fileoff as uintptr_t;

    let symoff_ptr = &(*symtab_cmd).symoff as *const u32;
    let symtab = (linkedit_base + symoff_ptr as uintptr_t) as *const arch::NlistT;
    let strtab = (linkedit_base + symoff_ptr as uintptr_t) as *const c_char;

    let indirect_symtab = (linkedit_base + (*dysymtab).indirectsymoff as uintptr_t) as *const u32;

    let mut cur_seg_cmd =
        header.byte_add(size_of::<MachHeaderT>() as uintptr_t) as *const arch::SegmentCommandT;
    for _ in 0..(*header).ncmds {
        if (*cur_seg_cmd).cmd == arch::LC_SEGMENT_ARCH_DEPENDENT {
            if !eq_u8((*cur_seg_cmd).segname, SEG_DATA)
                && !eq_u8((*cur_seg_cmd).segname, SEG_DATA_CONST)
            {
                cur_seg_cmd = cur_seg_cmd.byte_add((*cur_seg_cmd).cmdsize as uintptr_t);
                continue;
            }

            for j in 0..(*cur_seg_cmd).nsects {
                let sect = cur_seg_cmd.byte_add(size_of::<arch::SegmentCommandT>() + j as uintptr_t)
                    as *const arch::SectionT;

                println!("FLAGS: {}", (*sect).flags);

                if (*sect).flags & SECTION_TYPE == S_LAZY_SYMBOL_POINTERS {
                    perform_rebinding_with_section(sect, slide, symtab, strtab, indirect_symtab);
                }
                if (*sect).flags & SECTION_TYPE == S_NON_LAZY_SYMBOL_POINTERS {
                    perform_rebinding_with_section(sect, slide, symtab, strtab, indirect_symtab);
                }
            }
        }

        cur_seg_cmd = cur_seg_cmd.byte_add((*cur_seg_cmd).cmdsize as uintptr_t);
    }
}

unsafe fn perform_rebinding_with_section(
    sect: *const arch::SectionT,
    slide: c_int,
    symtab: *const arch::NlistT,
    strtab: *const c_char,
    indirect_symtab: *const u32,
) {
    println!("Performing rebinding with {}", slide);

    let indirect_symbol_indices = indirect_symtab.byte_add((*sect).reserved1 as uintptr_t);
    let indirect_symbol_bindings =
        (slide as uintptr_t + (*sect).addr as uintptr_t) as *mut *const c_void;

    for i in 0..(*sect).size as usize / size_of::<*const c_void>() {
        let symtab_index = indirect_symbol_indices.byte_add(i);
        if matches!(
            *symtab_index,
            INDIRECT_SYMBOL_ABS
                | INDIRECT_SYMBOL_LOCAL
                | (INDIRECT_SYMBOL_ABS | INDIRECT_SYMBOL_LOCAL)
        ) {
            continue;
        }

        println!("1 {}", *symtab_index);
        let strtab_offset = (*symtab.byte_add(symtab_index as uintptr_t)).n_strx;
        println!("2");
        let symbol_name = strtab.byte_add(strtab_offset as usize);

        for cur in BINDINGS.iter_mut() {
            let cur_name = &cur.name;
            if eq_char(symbol_name, cur_name) {
                let indirect_binding = indirect_symbol_bindings.wrapping_add(i) as *const c_void;

                if !cur.replacement.is_null() && indirect_binding != cur.replacement {
                    (*cur).replaced = indirect_binding;
                }

                println!("get protection");

                let result = mach_vm_protect(
                    mach_task_self(),
                    indirect_symbol_bindings as mach_vm_address_t,
                    (*sect).size as mach_vm_size_t,
                    0,
                    VM_PROT_READ | VM_PROT_WRITE | VM_PROT_COPY,
                );
                if result == KERN_SUCCESS {
                    *indirect_symbol_bindings.wrapping_add(i) = cur.replacement;
                }
            }
        }
    }
}

fn new_dl_info() -> Dl_info {
    Dl_info {
        dli_fname: null(),
        dli_fbase: null_mut(),
        dli_sname: null(),
        dli_saddr: null_mut(),
    }
}

#[inline]
fn eq_u8(a: impl IntoIterator<Item = u8>, b: &str) -> bool {
    a.into_iter().zip(b.as_bytes()).all(|(a, b)| a == *b)
}

#[inline]
unsafe fn eq_char(a: *const c_char, b: &str) -> bool {
    CStr::from_ptr(a).to_str().unwrap() == b
}
