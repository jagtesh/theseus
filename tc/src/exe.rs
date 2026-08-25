//! EXE loading.

use runtime::segofs;

use crate::{DOSModule, Export, Import, Module, WindowsModule, memory::Memory};

pub fn load_exe(mem: &mut Memory, buf: Vec<u8>) -> Module {
    match exe::parse(&buf).unwrap() {
        exe::Parse::PE(pe) => Module::Windows(load_pe(mem, &buf, pe)),
        exe::Parse::DOS(dos) => Module::DOS(load_dos(mem, &buf, dos)),
    }
}

fn load_dos(mem: &mut Memory, buf: &[u8], dos: exe::DOS) -> DOSModule {
    let psp_segment = dos::DOSBOX_SEG;

    mem.reserve("psp".into(), segofs(psp_segment, 0), 0x100);

    let load_segment = psp_segment + 0x10;
    let load_addr = segofs(load_segment, 0);
    let data = &buf[dos.image_offset()..];
    mem.reserve("dos data".into(), load_addr, data.len() as u32);
    mem.slice_mut(load_addr, data.len() as u32)
        .copy_from_slice(data);

    dos.apply_relocations(load_segment, &mut mem.bytes[load_addr as usize..]);

    DOSModule {
        is_com: false,
        psp_segment,
        load_segment: psp_segment + 0x10 + dos.header.e_cs,
        stack_segment: load_segment + dos.header.e_ss,
        stack_pointer: dos.header.e_sp,
        entry_point: dos.header.e_ip,
        code_memory: (load_addr..load_addr + data.len() as u32),
    }
}

pub fn load_pe(mem: &mut Memory, buf: &[u8], f: exe::PE) -> WindowsModule {
    if mem.mappings.vec().iter().all(|m| m.desc != "null page") {
        mem.mappings.alloc("null page".into(), 0x1000);
    }

    let image_base = f.opt_header.ImageBase;
    mem.reserve("exe header".into(), image_base, 0x1000);
    mem.write_bytes(image_base, &buf[..0x1000.min(buf.len())]);
    let mut code_range = None;
    let mut exec_ranges = Vec::new();
    for sec in &f.sections {
        let addr = image_base + sec.VirtualAddress;
        let size = runtime::round_to_page(sec.SizeOfRawData.max(sec.VirtualSize));
        mem.reserve(sec.name().unwrap().into(), addr, size);

        use exe::pe::IMAGE_SCN;
        let flags = sec.characteristics().unwrap();
        let load_data =
            flags.contains(IMAGE_SCN::CODE) || flags.contains(IMAGE_SCN::INITIALIZED_DATA);
        if load_data {
            let data = &buf[sec.PointerToRawData as usize..][..sec.SizeOfRawData as usize];
            mem.write_bytes(addr, data);
        }
        if flags.contains(IMAGE_SCN::CODE) || flags.contains(IMAGE_SCN::MEM_EXECUTE) {
            exec_ranges.push(addr..addr + size);
            match &mut code_range {
                None => code_range = Some(addr..addr + sec.SizeOfRawData),
                Some(range) => {
                    range.start = range.start.min(addr);
                    range.end = range.end.max(addr + sec.SizeOfRawData);
                }
            }
        }
    }
    exec_ranges.sort_by_key(|r| r.start);

    let resources = f
        .get_data_directory(exe::pe::IMAGE_DIRECTORY_ENTRY::RESOURCE)
        .map(|dir| {
            let addr = image_base + dir.VirtualAddress;
            addr..(addr + dir.Size)
        });

    let imports = read_imports(&f, mem);
    let exports = read_exports(&f, mem);

    WindowsModule {
        imports,
        image_base,
        entry_point: image_base + f.opt_header.AddressOfEntryPoint,
        code_memory: code_range.unwrap(),
        exec_ranges,
        resources,
        vtables: Default::default(),
        dynamic_exports: Default::default(),
        exports,
    }
}

/// Read the module's own exported symbols.
fn read_exports(pe_file: &exe::PE, mem: &Memory) -> Vec<Export> {
    let mut exports = vec![];
    let Some(dir) = pe_file.get_data_directory(exe::pe::IMAGE_DIRECTORY_ENTRY::EXPORT) else {
        return exports;
    };
    let image_base = pe_file.opt_header.ImageBase;
    let image = mem.slice_all(image_base);
    let table = exe::read_exports(dir.as_slice(image).unwrap());

    let mut names = std::collections::HashMap::new();
    for (name, index) in table.names(image) {
        names.insert(index, String::from_utf8_lossy(name).to_string());
    }

    let forwarders = dir.VirtualAddress..dir.VirtualAddress + dir.Size;
    for (i, rva) in table.fns(image).enumerate() {
        if rva == 0 {
            continue;
        }
        // A forwarder's slot holds not an address but an rva of the
        // "OTHERDLL.Symbol" string, which lands inside the export directory.
        if forwarders.contains(&rva) {
            continue;
        }
        exports.push(Export {
            ordinal: table.Base + i as u32,
            name: names.get(&(i as u16)).cloned(),
            addr: image_base + rva,
        });
    }
    exports
}

/// C++ libraries export mangled names -- mfc42 imports msvcrt's as
/// "?terminate@@YAXXZ" -- and these become Rust paths, so the characters that
/// are not identifier-legal are escaped rather than passed through.  The escape
/// is injective, so two distinct mangled names cannot collide.
pub(crate) fn escape_symbol(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for (i, c) in raw.chars().enumerate() {
        if c.is_ascii_alphanumeric() || c == '_' {
            if i == 0 && c.is_ascii_digit() {
                out.push('_');
            }
            out.push(c);
        } else {
            out.push_str(&format!("_x{:02x}", c as u32));
        }
    }
    out
}

fn is_data(dll: &str, func: &str) -> bool {
    if dll == "msvcrt" {
        return matches!(func, "_adjust_fdiv" | "_acmdln");
    }
    false
}

/// Read the file's imported symbols.
fn read_imports(pe_file: &exe::PE, mem: &Memory) -> Vec<Import> {
    let mut imports = vec![];
    let Some(dir) = pe_file.get_data_directory(exe::pe::IMAGE_DIRECTORY_ENTRY::IMPORT) else {
        return imports;
    };
    let image_base = pe_file.opt_header.ImageBase;
    let image = mem.slice_all(image_base);
    for imp in exe::read_imports(dir.as_slice(image).unwrap()) {
        let name = std::str::from_utf8(imp.image_name(image))
            .unwrap()
            .to_lowercase();
        let name = name.trim_end_matches(".dll");
        for (addr, entry) in imp.iat_iter(image) {
            let func = match entry.as_import_symbol(image) {
                exe::ImportSymbol::Name(name) => {
                    escape_symbol(std::str::from_utf8(name).unwrap())
                }
                exe::ImportSymbol::Ordinal(n) => format!("ordinal{n}"),
            };
            let data = is_data(name, &func);
            imports.push(Import {
                dll: name.to_string(),
                func,
                iat_addr: image_base + addr,
                addr: 0,
                data,
                linked: false,
            });
        }
    }
    imports
}

/// Load an additional PE into the same address space as the main module, so its
/// code can be translated into the same block table. Windows DLLs carry distinct
/// preferred bases, so nothing here relocates them.
pub fn load_module(mem: &mut Memory, buf: Vec<u8>) -> anyhow::Result<WindowsModule> {
    match exe::parse(&buf).unwrap() {
        exe::Parse::PE(pe) => Ok(load_pe(mem, &buf, pe)),
        exe::Parse::DOS(_) => anyhow::bail!("--link expects a PE module"),
    }
}
