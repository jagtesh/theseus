//! The message loop, served by Sogen's win32k.
//!
//! The queue, the hit-testing that turns a host mouse event into WM_NCHITTEST/WM_MOUSEMOVE, and the
//! dispatch back into the window procedure are all the kernel's. Nothing here models a message.

use std::sync::LazyLock;

use runtime::Context;

use crate::{POINT, Ptr, sogen, stub, trace, user32::{HACCEL, HWND}};

/// If THESEUS_TRACE includes "wm", log all Windows messages.
static LOG_MESSAGES: LazyLock<bool> =
    LazyLock::new(|| !matches!(trace::get_uncached("wm"), trace::Trace::None));

pub type WPARAM = u32;
pub type LPARAM = u32;

#[derive(win32_derive::ABIEnum, Debug)]
pub enum WM {
    PAINT = 0xf,
    QUIT = 0x12,
    MOUSEMOVE = 0x200,
    LBUTTONDOWN = 0x201,
    LBUTTONUP = 0x202,
    RBUTTONDOWN = 0x204,
    RBUTTONUP = 0x205,
    MBUTTONDOWN = 0x207,
    MBUTTONUP = 0x208,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, zerocopy::FromBytes, zerocopy::IntoBytes, zerocopy::Immutable)]
pub struct MSG {
    hwnd: HWND,
    message: u32,
    wParam: WPARAM,
    lParam: LPARAM,
    time: u32,
    pt: POINT,
}

const PM_REMOVE: u32 = 1;

/// The kernel's GetMessage parks the calling thread until a message arrives, and a native client has
/// no scheduler to park on. Polling PeekMessage is the same observable sequence without the block.
fn wait_for_message(ctx: &mut Context, msg_addr: u32, hWnd: HWND, remove: u32) -> bool {
    loop {
        sogen::pump();
        let got = sogen::with_context(ctx, |_| {
            sogen::peek_message(msg_addr, hWnd.to_raw(), 0, 0, remove)
        });

        if got != 0 {
            return true;
        }

        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

#[win32_derive::dllexport]
pub fn GetMessageA(
    ctx: &mut Context,
    lpMsg: Ptr<MSG>,
    hWnd: HWND,
    wMsgFilterMin: u32,
    wMsgFilterMax: u32,
) -> i32 {
    GetMessageW(ctx, lpMsg, hWnd, wMsgFilterMin, wMsgFilterMax)
}

#[win32_derive::dllexport]
pub fn GetMessageW(
    ctx: &mut Context,
    lpMsg: Ptr<MSG>,
    hWnd: HWND,
    _wMsgFilterMin: u32,
    _wMsgFilterMax: u32,
) -> i32 {
    let msg_addr = lpMsg.addr;
    wait_for_message(ctx, msg_addr, hWnd, PM_REMOVE);

    let message = ctx.memory.read::<u32>(msg_addr + 4);
    if *LOG_MESSAGES {
        log::info!("GetMessage -> {message:#x}");
    }

    if message == WM::QUIT as u32 { 0 } else { 1 }
}

#[win32_derive::dllexport]
pub fn PeekMessageA(
    ctx: &mut Context,
    lpMsg: Ptr<MSG>,
    hWnd: HWND,
    _wMsgFilterMin: u32,
    _wMsgFilterMax: u32,
    wRemoveMsg: u32, /* PEEK_MESSAGE_REMOVE_TYPE */
) -> bool {
    sogen::pump();
    sogen::with_context(ctx, |_| {
        sogen::peek_message(lpMsg.addr, hWnd.to_raw(), 0, 0, wRemoveMsg)
    }) != 0
}

#[win32_derive::dllexport]
pub fn PeekMessageW(
    ctx: &mut Context,
    lpMsg: Ptr<MSG>,
    hWnd: HWND,
    wMsgFilterMin: u32,
    wMsgFilterMax: u32,
    wRemoveMsg: u32,
) -> bool {
    PeekMessageA(ctx, lpMsg, hWnd, wMsgFilterMin, wMsgFilterMax, wRemoveMsg)
}

#[win32_derive::dllexport]
pub fn TranslateMessage(ctx: &mut Context, lpMsg: Ptr<MSG>) -> bool {
    sogen::with_context(ctx, |_| sogen::translate_message(lpMsg.addr, 0)) != 0
}

#[win32_derive::dllexport]
pub fn DispatchMessageA(ctx: &mut Context, lpMsg: Ptr<MSG>) -> u32 {
    DispatchMessageW(ctx, lpMsg)
}

#[win32_derive::dllexport]
pub fn DispatchMessageW(ctx: &mut Context, lpMsg: Ptr<MSG>) -> u32 {
    // The kernel routes this to the window procedure through the client callback, which lands back
    // in this process as an ordinary call.
    sogen::with_context(ctx, |_| sogen::dispatch_message(lpMsg.addr))
}

#[win32_derive::dllexport]
pub fn TranslateAcceleratorW(
    _ctx: &mut Context,
    _hWnd: HWND,
    _hAccTable: HACCEL,
    _lpMsg: Ptr<MSG>,
) -> i32 {
    stub!(0) // no accelerator table is built, so nothing translates
}

#[win32_derive::dllexport]
pub fn PostQuitMessage(ctx: &mut Context, nExitCode: i32) {
    // NtUserCallOneParam(exit code, POSTQUITMESSAGE).
    sogen::with_context(ctx, |_| sogen::call_one_param(nExitCode as u32, 51));
}

#[win32_derive::dllexport]
pub fn PostMessageW(
    _ctx: &mut Context,
    _hWnd: HWND,
    _Msg: u32,
    _wParam: WPARAM,
    _lParam: LPARAM,
) -> bool {
    stub!(true)
}

#[win32_derive::dllexport]
pub fn SendMessageW(
    ctx: &mut Context,
    hWnd: HWND,
    Msg: u32,
    wParam: WPARAM,
    lParam: LPARAM,
) -> u32 {
    // FNID_SENDMESSAGE.
    sogen::with_context(ctx, |_| {
        sogen::message_call(hWnd.to_raw(), Msg, wParam, lParam, 0, 0x2B1, 0)
    })
}
