use crate::arch::{MachHeaderT, NlistT, SectionT, SegmentCommandT};
use goblin::mach::constants::{
    SECTION_TYPE, SEG_DATA, SEG_LINKEDIT, S_LAZY_SYMBOL_POINTERS, S_NON_LAZY_SYMBOL_POINTERS,
};
use goblin::mach::load_command::{DysymtabCommand, SymtabCommand, LC_DYSYMTAB, LC_SYMTAB};
use mach2::kern_return::KERN_SUCCESS;
use mach2::traps::mach_task_self;
use mach2::vm::mach_vm_protect;
use mach2::vm_prot::{VM_PROT_COPY, VM_PROT_READ, VM_PROT_WRITE};
use mach2::vm_types::{mach_vm_address_t, mach_vm_size_t};
use std::ffi::{c_char, c_int, c_void, CStr};

static mut BINDINGS: Vec<Rebinding> = Vec::new();

const SEG_DATA_CONST: &str = "__DATA_CONST";

#[cfg(target_pointer_width = "64")]
mod arch {
    use goblin::mach::header::Header64;
    use goblin::mach::load_command::{Section64, SegmentCommand64, LC_SEGMENT_64};
    use goblin::mach::symbols::Nlist64;

    pub const LC_SEGMENT_ARCH_DEPENDENT: u32 = LC_SEGMENT_64;
    pub type NlistT = Nlist64;
    pub type SectionT = Section64;
    pub type SegmentCommandT = SegmentCommand64;
    pub type MachHeaderT = Header64;
}

#[cfg(target_pointer_width = "32")]
mod arch {
    use goblin::mach::header::Header32;
    use goblin::mach::load_command::{Section32, SegmentCommand32, LC_SEGMENT, LC_SEGMENT_64};
    use goblin::mach::symbols::Nlist32;

    pub const LC_SEGMENT_ARCH_DEPENDENT: u32 = LC_SEGMENT;
    pub type NlistT = Nlist32;
    pub type SectionT = Section32;
    pub type SegmentCommandT = SegmentCommand32;
    pub type MachHeaderT = Header32;
}

extern "C" {
    fn _dyld_register_func_for_add_image(callback: extern "C" fn(*const c_void, c_int));
}

#[derive(Clone)]
pub struct Rebinding {
    pub name: String,
    pub function: *const c_void,
}

pub unsafe fn register(bindings: Vec<Rebinding>) {
    BINDINGS = bindings;

    _dyld_register_func_for_add_image(add_image);
}

extern "C" fn add_image(header: *const c_void, slide: c_int) {
    unsafe {
        rebind_for_image(header, slide);
    }
}

unsafe fn rebind_for_image(header: *const c_void, slide: c_int) {
    let mut linked_segment_cmd = None;
    let mut symtab_cmd = None;
    let mut dynsymtab_cmd = None;

    let header = header as *const MachHeaderT;

    let mut segment_cmd = header.byte_add(size_of::<MachHeaderT>()) as *const SegmentCommandT;

    for _ in 0..(*header).ncmds {
        match (*segment_cmd).cmd {
            arch::LC_SEGMENT_ARCH_DEPENDENT => {
                if equal_str((*segment_cmd).segname, SEG_LINKEDIT) {
                    linked_segment_cmd = Some(segment_cmd);
                }
            }
            LC_SYMTAB => {
                symtab_cmd = Some(segment_cmd as *const SymtabCommand);
            }
            LC_DYSYMTAB => {
                dynsymtab_cmd = Some(segment_cmd as *const DysymtabCommand);
            }
            _ => {}
        }

        segment_cmd = segment_cmd.byte_add((*segment_cmd).cmdsize as usize);
    }

    let (Some(symtab_cmd), Some(dynsymtab_cmd)) = (symtab_cmd, dynsymtab_cmd) else {
        return;
    };

    if (*dynsymtab_cmd).nindirectsyms == 0 {
        return;
    }

    let mut segment_cmd = header.byte_add(size_of::<MachHeaderT>()) as *const SegmentCommandT;

    for _ in 0..(*header).ncmds {
        if (*segment_cmd).cmd == arch::LC_SEGMENT_ARCH_DEPENDENT
            && (equal_str((*segment_cmd).segname, SEG_DATA)
                || equal_str((*segment_cmd).segname, SEG_DATA_CONST))
        {
            bind_symbols(header, segment_cmd, symtab_cmd, dynsymtab_cmd, slide);
        }

        segment_cmd = segment_cmd.byte_add((*segment_cmd).cmdsize as usize);
    }
}

unsafe fn bind_symbols(
    header: *const MachHeaderT,
    segment_cmd: *const SegmentCommandT,
    symtab_cmd: *const SymtabCommand,
    dynsymtab_cmd: *const DysymtabCommand,
    slide: i32,
) {
    let symbol_table = header.byte_add((*symtab_cmd).symoff as usize) as *const NlistT;
    let indirect_symbol_table =
        header.byte_add((*dynsymtab_cmd).indirectsymoff as usize) as *const u32;

    for j in 0..(*segment_cmd).nsects {
        let sect = segment_cmd
            .byte_add(size_of::<SegmentCommandT>() + j as usize * size_of::<SectionT>())
            as *const SectionT;

        if !matches!(
            (*sect).flags & SECTION_TYPE,
            S_NON_LAZY_SYMBOL_POINTERS | S_LAZY_SYMBOL_POINTERS
        ) {
            continue;
        }

        let indirect_bindings = ((*sect).addr as usize + slide as usize) as *mut *const c_void;

        let len_sect = (*sect).size as usize / size_of::<*const c_void>();
        'symbol_loop: for k in 0..len_sect {
            let symbol_index = *indirect_symbol_table.add(k);

            if symbol_index >= (*symtab_cmd).nsyms {
                continue;
            }

            let symbol = symbol_table.add(symbol_index as usize) as *const NlistT;

            let string_table_end = (*symtab_cmd).stroff as usize + (*symtab_cmd).strsize as usize;
            let offset = (*symtab_cmd).stroff as usize + (*symbol).n_strx as usize;
            if offset >= string_table_end {
                continue;
            }

            let symbol_name = header
                .byte_add((*symtab_cmd).stroff as usize + (*symbol).n_strx as usize)
                as *const c_char;

            let Ok(name) = CStr::from_ptr(symbol_name).to_str() else {
                continue;
            };

            if name.is_empty() {
                continue;
            }

            let indirect_binding = indirect_bindings.wrapping_add(k as usize) as *const c_void;

            for binding in BINDINGS.iter_mut() {
                if name[1..] == binding.name {
                    if binding.function.is_null() || indirect_binding == binding.function {
                        continue;
                    }

                    let result = mach_vm_protect(
                        mach_task_self(),
                        indirect_bindings as mach_vm_address_t,
                        (*sect).size as mach_vm_size_t,
                        0,
                        VM_PROT_READ | VM_PROT_WRITE | VM_PROT_COPY,
                    );
                    if result == KERN_SUCCESS {
                        *indirect_bindings.wrapping_add(k as usize) = binding.function;
                    }
                    continue 'symbol_loop;
                }
            }
        }
    }
}

#[inline]
fn equal_str(a: impl IntoIterator<Item = u8>, b: &str) -> bool {
    a.into_iter().zip(b.as_bytes()).all(|(a, &b)| a == b)
}
