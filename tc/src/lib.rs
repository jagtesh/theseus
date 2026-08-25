use std::collections::HashMap;

use crate::memory::Memory;

mod codegen;
pub mod com;
pub mod exe;
mod gather;
mod memory;
pub use gather::{EntryPoint, Gather, IP};
use runtime::segofs;

#[derive(Default)]
pub struct DOSModule {
    pub is_com: bool,
    // initial register values
    pub psp_segment: u16,
    pub load_segment: u16,
    pub stack_segment: u16,
    pub stack_pointer: u16,
    pub entry_point: u16,
    pub code_memory: std::ops::Range<u32>,
}

#[derive(Default)]
pub struct WindowsModule {
    pub image_base: u32,
    pub entry_point: u32,
    pub code_memory: std::ops::Range<u32>,
    /// Mapped ranges of the sections marked executable, sorted by address.
    /// Distinct from code_memory, which spans them and any data section between.
    pub exec_ranges: Vec<std::ops::Range<u32>>,
    pub resources: Option<std::ops::Range<u32>>,
    pub imports: Vec<Import>,
    pub vtables: Vec<(String, u32)>,
    /// (dll, function) pairs the program may resolve through GetProcAddress.
    pub dynamic_exports: Vec<(String, String)>,
    /// The module's own PE export table.
    pub exports: Vec<Export>,
}

#[derive(Debug, Clone)]
pub struct Export {
    pub ordinal: u32,
    /// Only a minority of exports are named: mfc42 names 6 of its 6,389.
    pub name: Option<String>,
    pub addr: u32,
}

impl Export {
    /// Name for the generated function, unique across the export table.
    pub fn ident(&self) -> String {
        match &self.name {
            Some(name) => exe::escape_symbol(name),
            None => format!("ord_{}", self.ordinal),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Import {
    pub dll: String,
    pub func: String,
    /// address to write func_addr to
    pub iat_addr: u32,
    /// address of code/data
    pub addr: u32,
    /// when true, data, not code
    pub data: bool,
    /// when true, bound to a linked module's export, so the translated code at
    /// `addr` is the implementation and no host stub stands in for it
    pub linked: bool,
}

pub enum Module {
    DOS(DOSModule),
    Windows(WindowsModule),
}

impl Module {
    // TODO: remove some of these methods as we untangle DOS vs Windows
    pub fn bitness(&self) -> u32 {
        match self {
            Module::DOS(_) => 16,
            Module::Windows(_) => 32,
        }
    }
    fn is_dos(&self) -> bool {
        matches!(self, Module::DOS(_))
    }
    fn is_windows(&self) -> bool {
        matches!(self, Module::Windows(_))
    }
    pub fn segment_addressed(&self) -> bool {
        self.is_dos()
    }

    pub fn local_addr(&self, addr: u32) -> IP {
        match self {
            Module::DOS(m) => IP::Seg((m.load_segment, addr as u16).into()),
            Module::Windows(_) => IP::Flat(addr),
        }
    }

    fn entry_point(&self) -> IP {
        let local = match self {
            Module::DOS(m) => m.entry_point as u32,
            Module::Windows(m) => m.entry_point,
        };
        self.local_addr(local)
    }

    fn image_base(&self) -> u32 {
        match self {
            Module::DOS(m) => segofs(m.psp_segment, 0),
            Module::Windows(m) => m.image_base,
        }
    }

    fn code_memory(&self) -> std::ops::Range<u32> {
        match self {
            Module::DOS(m) => m.code_memory.clone(),
            Module::Windows(m) => m.code_memory.clone(),
        }
    }

    /// The executable region containing addr, if any.  A DOS image has no
    /// section table to consult, so its whole loaded image counts.
    pub fn exec_range(&self, addr: u32) -> Option<std::ops::Range<u32>> {
        match self {
            Module::DOS(m) => m.code_memory.contains(&addr).then(|| m.code_memory.clone()),
            Module::Windows(m) => m
                .exec_ranges
                .iter()
                .find(|r| r.contains(&addr))
                .cloned(),
        }
    }
}

#[derive(Default)]
pub struct AddrInfo {
    pub name: String,
    pub is_extern: bool,
}

pub struct State {
    pub module: Module,
    /// Additional modules loaded into the same address space, keyed by the name
    /// an import refers to them by (lowercased, without extension). Their code is
    /// translated alongside the main module's, so an import of one of them binds
    /// to a real address rather than a synthetic one.
    pub linked: Vec<(String, WindowsModule)>,
    pub mem: Memory,
    pub addr_info: HashMap<u32, AddrInfo>,
    pub blocks: HashMap<u32, Block>,
    pub report: gather::Report,
}

impl Default for State {
    fn default() -> Self {
        Self {
            module: Module::DOS(DOSModule::default()),
            linked: Default::default(),
            mem: Default::default(),
            addr_info: Default::default(),
            blocks: Default::default(),
            report: Default::default(),
        }
    }
}

pub struct Instr {
    pub ip: IP,
    pub iced: iced_x86::Instruction,
    pub hint: Option<String>,
}

impl Instr {
    pub fn next_ip(&self) -> IP {
        self.ip.with_local(self.iced.next_ip32())
    }
}

pub struct Block {
    name: Option<String>,
    ty: BlockType,
}

pub enum BlockType {
    Instrs(Vec<Instr>),
    Stdcall(String),
    Extern(u32), // TODO: use ip instead
}

impl Block {
    pub fn name(&self) -> String {
        if let Some(name) = &self.name {
            return name.clone();
        }
        match &self.ty {
            BlockType::Instrs(instrs) => match instrs[0].ip {
                IP::Flat(addr) => format!("x{:x}", addr),
                IP::Seg(addr) => format!("x{:04x}_{:04x}", addr.seg, addr.ofs),
            },
            BlockType::Stdcall(func) => format!("{}_stdcall", func),
            BlockType::Extern(ip) => format!("x{:x}", ip),
        }
    }
}

pub fn write_if_changed(path: &str, contents: &[u8]) -> anyhow::Result<()> {
    let existing = std::fs::read(&path).unwrap_or_default();
    if existing != contents {
        std::fs::write(path, contents)?;
    }
    Ok(())
}

impl State {
    pub fn load_symbols(&mut self, csv: impl std::io::Read) -> anyhow::Result<()> {
        let mut rdr = csv::Reader::from_reader(csv);
        for result in rdr.records() {
            let record = result?;
            let name = &record[0];
            if name.starts_with("FUN_") {
                continue;
            }
            let addr = &record[1];
            if !addr.starts_with('0') {
                continue;
            }
            let addr = u32::from_str_radix(&addr, 16)
                .map_err(|err| anyhow::anyhow!(format!("{addr:?}: {err}")))?;
            self.addr_info.insert(
                addr,
                AddrInfo {
                    name: name.to_string(),
                    is_extern: false,
                },
            );
        }
        Ok(())
    }

    /// For any dll used by the module, reserve executable-memory addresses for
    /// the things it can't import statically: COM vtable entries, and the
    /// functions it may look up through GetProcAddress.
    fn add_vtables(&mut self) -> u32 {
        let Module::Windows(module) = &mut self.module else {
            unreachable!()
        };
        let mut addr = 0; // only set up if vtables are needd
        for (dll, vtables) in [
            ("ddraw", winapi::ddraw::VTABLES.as_slice()),
            ("dsound", winapi::dsound::VTABLES.as_slice()),
            ("dinput", winapi::dinput::VTABLES.as_slice()),
        ] {
            if !module.imports.iter().any(|imp| imp.dll == dll) {
                continue;
            }
            if addr == 0 {
                addr = self.mem.mappings.alloc("vtables".into(), 0x1000);
                assert!(addr != 0);
            }
            for (interface, entries) in vtables {
                module.vtables.push((format!("{dll}::{interface}"), addr));
                for func in entries.iter() {
                    module.imports.push(Import {
                        dll: dll.to_string(),
                        func: format!("{interface}::{func}"),
                        iat_addr: addr,
                        addr: 0,
                        data: false,
                        linked: false,
                    });
                    addr += 4;
                }
            }
        }

        for (dll, funcs) in winapi::DYNAMIC_EXPORTS {
            if !module.imports.iter().any(|imp| imp.dll == *dll) {
                continue;
            }
            for func in funcs.iter() {
                module
                    .dynamic_exports
                    .push((dll.to_string(), func.to_string()));
                // A function already imported statically has an address
                // already; only the rest need one reserved.
                if module
                    .imports
                    .iter()
                    .any(|imp| imp.dll == *dll && imp.func == *func)
                {
                    continue;
                }
                if addr == 0 {
                    addr = self.mem.mappings.alloc("vtables".into(), 0x1000);
                    assert!(addr != 0);
                }
                module.imports.push(Import {
                    dll: dll.to_string(),
                    func: func.to_string(),
                    iat_addr: addr,
                    addr: 0,
                    data: false,
                    linked: false,
                });
                addr += 4;
            }
        }

        addr
    }

/// Bind an import to a linked module's export, if that module was supplied.
/// Most of a DLL's exports are unnamed, so imports by ordinal are the common
/// case: mfc42 names 6 of its 6,389.
fn resolve_linked(linked: &[(String, WindowsModule)], import: &Import) -> Option<u32> {
    let dll = import
        .dll
        .rsplit_once('.')
        .map_or(import.dll.as_str(), |(stem, _)| stem)
        .to_ascii_lowercase();
    let (_, module) = linked.iter().find(|(name, _)| *name == dll)?;
    let export = match import.func.strip_prefix("ordinal") {
        Some(n) => {
            let ordinal: u32 = n.parse().ok()?;
            module.exports.iter().find(|e| e.ordinal == ordinal)?
        }
        // Import names are escaped into legal Rust identifiers when they are read,
        // while exports keep the raw name, so the comparison has to escape too:
        // MFC imports msvcrt by mangled name, e.g. `?terminate@@YAXXZ`.
        None => module.exports.iter().find(|e| {
            e.name
                .as_deref()
                .is_some_and(|name| exe::escape_symbol(name) == import.func)
        })?,
    };
    Some(export.addr)
}

    fn write_iat(&mut self, data_addr: u32) {
        let Module::Windows(module) = &mut self.module else {
            unreachable!()
        };
        let mut data_addr = data_addr;
        let mut func_addr = 0xfafbfc00;
        for import in module.imports.iter_mut() {
            if import.iat_addr == 0 {
                panic!("{import:#x?}");
            }
            if let Some(addr) = Self::resolve_linked(&self.linked, import) {
                import.addr = addr;
                import.linked = true;
            } else if import.data {
                import.addr = data_addr;
                data_addr += 4;
            } else {
                import.addr = func_addr;
                func_addr += 1;
            }
            self.mem.write::<u32>(import.iat_addr, import.addr);
        }
    }

    pub fn init_imports(&mut self) {
        if matches!(self.module, Module::Windows(_)) {
            let data_addr = self.add_vtables();
            self.write_iat(data_addr);
        }
    }

    /// Install externs for ambient addresses that make system calls.
    pub fn init_system_hooks(&mut self) {
        match &self.module {
            Module::DOS(m) => {
                if m.is_com {
                    self.addr_info.insert(
                        0,
                        AddrInfo {
                            name: "dos::exit".into(),
                            is_extern: true,
                        },
                    );
                }
            }
            Module::Windows(_) => {}
        }
    }

    pub fn gather(&mut self, gather: Gather) {
        let (blocks, report) = gather.run(self);
        self.blocks = blocks;
        self.report = report;
    }

    pub fn generate(mut self, trace: bool, out_dir: &str) -> anyhow::Result<()> {
        let mut codegen = codegen::CodeGen::new(&mut self, trace);
        codegen.gen_file(out_dir)?;

        let data_dir = format!("{out_dir}/data");
        std::fs::create_dir_all(&data_dir)?;
        for map in self.mem.mappings.vec().iter() {
            let buf = self.mem.slice(map.addr, map.size);
            if buf.iter().all(|&b| b == 0) {
                continue;
            }
            write_if_changed(&format!("{out_dir}/data/{:08x}.raw", map.addr), buf)?;
        }

        let mut report = self.report;
        report.name = out_dir.to_string();
        let report_path = format!("{out_dir}/report.html");
        write_if_changed(&report_path, report.to_html().as_bytes())?;

        fn link_path(path: &str) -> String {
            let abs_path = std::env::current_dir()
                .unwrap()
                .join(path)
                .to_string_lossy()
                .to_string();
            format!("\x1b]8;;file://{abs_path}\x1b\\{path}\x1b]8;;\x1b\\")
        }

        println!(
            "generated in {out_dir}; report in {link}",
            link = link_path(&report_path),
        );

        Ok(())
    }
}
