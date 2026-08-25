mod dialog;
mod message;
mod misc;
mod rect;
mod resource;
mod window;

use std::{
    cell::{OnceCell, RefCell},
    rc::Rc,
};

pub use dialog::*;
pub use message::*;
pub use misc::*;
pub use rect::*;
pub use resource::*;
pub use window::*;

use crate::HANDLE;

pub type HWND = HANDLE;
pub type HMENU = u32;
pub type HINSTANCE = u32;
pub type HCURSOR = u32;
pub type HICON = u32;
pub type HACCEL = u32;

/// Window state now lives in win32k, and nothing constructs this any more; the type survives
/// because ddraw, which is not wired to win32k, still names its fields.
pub struct Window {
    pub hwnd: HWND,
    pub dirty: bool,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub pixels: Option<u32>,
    pub host: host::Window,
    pub surface: Option<host::Surface>,
}

impl Window {
    pub fn resize(&mut self, _ctx: &mut runtime::Context, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.host.resize(width, height);
    }
}

pub struct State {
    pub wndclass: RefCell<Option<WndClass>>,
    pub window: RefCell<Option<Rc<RefCell<Window>>>>,
}

// TODO: reuse locking pattern from kernel32
// XXX sdl is not thread-safe so we cannot put it in a Mutex anyway, argh
struct StaticState(OnceCell<State>);
unsafe impl Sync for StaticState {}

static STATE: StaticState = StaticState(OnceCell::new());

pub fn state() -> &'static State {
    STATE.0.get_or_init(|| State {
        window: Default::default(),
        wndclass: Default::default(),
    })
}
