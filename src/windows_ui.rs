//! Win32 pieces of the Windows screensaver build: the `/c` settings dialog and the single-instance guard.
//! This is the only module of the crate that allows `unsafe` (it calls into the Win32 API).
#![allow(unsafe_code)]

use std::num::NonZeroIsize;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicU32, Ordering};

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::raw_window_handle::{RawWindowHandle, Win32WindowHandle};
use winit::window::{Window, WindowAttributes};

use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    COLOR_BTNFACE, CreateFontIndirectW, DEFAULT_GUI_FONT, DeleteObject, GetStockObject, HBRUSH, HGDIOBJ,
    MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY, MonitorFromPoint, MonitorFromWindow, UpdateWindow,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForMonitor, MDT_EFFECTIVE_DPI,
    SetProcessDpiAwarenessContext, SystemParametersInfoForDpi,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CreateWindowExW, DefWindowProcW,
    DestroyWindow, DispatchMessageW, GetClientRect, GetDlgItem, GetMessageW, GetSystemMetrics, GetWindowRect,
    IDC_ARROW, IDCANCEL, IDOK, IsDialogMessageW, IsWindow, LoadCursorW, MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MSG,
    MessageBoxW, NONCLIENTMETRICSW, PostQuitMessage, RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SPI_GETNONCLIENTMETRICS,
    SW_SHOW, SendMessageW, SetForegroundWindow, ShowWindow, TranslateMessage, UnregisterClassW, WM_CLOSE, WM_COMMAND,
    WM_DESTROY, WM_SETFONT, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_EX_DLGMODALFRAME, WS_POPUP, WS_SYSMENU, WS_TABSTOP,
    WS_VISIBLE,
};

const TITLE: &str = "Open Scrantic - Settings";
const CLASS: &str = "OpenScranticSettings";
const LABELS: [&str; 4] = ["Background", "Clouds", "Waves", "Sounds"];
/// Control id of the first checkbox; the others follow.
const FIRST_CHECKBOX: i32 = 101;
/// Set in [`RESULT`] when the user pressed OK; the low bits hold the four checkboxes.
const ACCEPTED: u32 = 1 << 8;
const CHECKED: usize = 1;

const CLIENT_W: i32 = 280;
const CLIENT_H: i32 = 188;
/// `SS_CENTER`: static text centered in its box.
const SS_CENTER: u32 = 1;
const CHECKBOX_W: i32 = 110;
const BUTTON_W: i32 = 76;
const BUTTON_GAP: i32 = 12;

/// Outcome of the dialog, shared with the window procedure (there is one dialog per process).
static RESULT: AtomicU32 = AtomicU32::new(0);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// `v`, a length at 96 DPI, at `dpi`, rounded to the nearest pixel.
fn scaled(v: i32, dpi: u32) -> i32 {
    (v * dpi as i32 + 48) / 96
}

/// DPI of the monitor `owner` is on, or of the primary monitor without an owner; 96 if it cannot be told.
/// (The monitor's, not the window's: a window of a DPI-unaware process reports 96 wherever it is.)
unsafe fn monitor_dpi(owner: HWND) -> u32 {
    // SAFETY: the monitor handle comes from the calls just before; both out values are valid for the call.
    unsafe {
        let monitor = if owner.is_null() {
            MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY)
        } else {
            MonitorFromWindow(owner, MONITOR_DEFAULTTONEAREST)
        };
        let (mut x, mut y) = (0, 0);
        if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y) == 0 && x != 0 {
            x
        } else {
            96
        }
    }
}

/// The font Windows uses for its own dialogs (Segoe UI 9 pt) at `dpi`, or null if it cannot be queried. The
/// caller deletes it.
unsafe fn system_message_font(dpi: u32) -> HGDIOBJ {
    // SAFETY: `metrics` is valid for the call and `cbSize` tells Windows how much of it to fill.
    unsafe {
        let mut metrics: NONCLIENTMETRICSW = std::mem::zeroed();
        metrics.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;
        let ok = SystemParametersInfoForDpi(
            SPI_GETNONCLIENTMETRICS,
            metrics.cbSize,
            (&raw mut metrics).cast(),
            0,
            dpi,
        );
        if ok == 0 {
            return null_mut();
        }
        CreateFontIndirectW(&metrics.lfMessageFont)
    }
}

/// Creates one child control of `parent`.
#[allow(clippy::too_many_arguments)]
unsafe fn child(parent: HWND, class: &str, text: &str, style: u32, rect: [i32; 4], id: i32, font: usize) -> HWND {
    let (class, text) = (wide(class), wide(text));
    let [x, y, w, h] = rect;
    // SAFETY: `class` and `text` are NUL-terminated and outlive the call; `parent` is a live window.
    unsafe {
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            text.as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            x,
            y,
            w,
            h,
            parent,
            id as usize as _,
            GetModuleHandleW(null()),
            null(),
        );
        SendMessageW(hwnd, WM_SETFONT, font, 1);
        hwnd
    }
}

/// Bit `i` is set when checkbox `i` is checked.
unsafe fn read_checks(hwnd: HWND) -> u32 {
    (0..LABELS.len() as i32).fold(0, |bits, i| {
        // SAFETY: the checkboxes are children of `hwnd`, created before the message loop starts.
        let checked = unsafe { SendMessageW(GetDlgItem(hwnd, FIRST_CHECKBOX + i), BM_GETCHECK, 0, 0) };
        bits | (u32::from(checked as usize == CHECKED) << i)
    })
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: called by the system with the arguments of a message for `hwnd`.
    unsafe {
        match msg {
            WM_COMMAND => {
                match (wparam & 0xFFFF) as i32 {
                    IDOK => {
                        RESULT.store(read_checks(hwnd) | ACCEPTED, Ordering::Relaxed);
                        DestroyWindow(hwnd);
                    }
                    IDCANCEL => {
                        DestroyWindow(hwnd);
                    }
                    _ => {}
                }
                0
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// Shows the modal settings dialog with the Background, Clouds, Waves and Sounds checkboxes preset to
/// `initial`. Returns the new values on OK, `None` on Cancel (or if the dialog cannot be created).
/// `owner` is the window handle Windows passes after `/c:`; it is disabled while the dialog is open.
pub fn settings_dialog(initial: [bool; 4], owner: Option<isize>) -> Option<[bool; 4]> {
    RESULT.store(0, Ordering::Relaxed);
    let class = wide(CLASS);
    // SAFETY: plain Win32 calls; every pointer passed is valid for the duration of its call.
    unsafe {
        // Without this Windows stretches the dialog on screens scaled above 100%, which blurs it. This process
        // shows nothing else; a run gets the same awareness from winit.
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(null());
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_BTNFACE + 1) as usize as HBRUSH,
            lpszMenuName: null(),
            lpszClassName: class.as_ptr(),
        };
        if RegisterClassW(&wc) == 0 {
            return None;
        }
        let owner = owner.map_or(null_mut(), |h| h as usize as HWND);
        let owner = if !owner.is_null() && IsWindow(owner) != 0 {
            owner
        } else {
            null_mut()
        };

        let dpi = monitor_dpi(owner);
        let px = |v: i32| scaled(v, dpi);
        let (cw, ch) = (px(CLIENT_W), px(CLIENT_H));
        let style = WS_POPUP | WS_CAPTION | WS_SYSMENU;
        let mut frame = RECT {
            left: 0,
            top: 0,
            right: cw,
            bottom: ch,
        };
        AdjustWindowRectExForDpi(&mut frame, style, 0, WS_EX_DLGMODALFRAME, dpi);
        let (w, h) = (frame.right - frame.left, frame.bottom - frame.top);
        let title = wide(TITLE);
        // Centered over the owner (Screen Saver Settings) when there is one, else on the screen.
        let mut area = RECT {
            left: 0,
            top: 0,
            right: GetSystemMetrics(SM_CXSCREEN),
            bottom: GetSystemMetrics(SM_CYSCREEN),
        };
        if !owner.is_null() {
            GetWindowRect(owner, &mut area);
        }
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            class.as_ptr(),
            title.as_ptr(),
            style,
            (area.left + area.right - w) / 2,
            (area.top + area.bottom - h) / 2,
            w,
            h,
            owner,
            null_mut(),
            instance,
            null(),
        );
        if hwnd.is_null() {
            UnregisterClassW(class.as_ptr(), instance);
            return None;
        }

        let message_font = system_message_font(dpi);
        let font = if message_font.is_null() {
            GetStockObject(DEFAULT_GUI_FONT)
        } else {
            message_font
        } as usize;
        let margin = px(16);
        child(
            hwnd,
            "STATIC",
            "Choose which features are enabled:",
            SS_CENTER,
            [margin, px(14), cw - 2 * margin, px(18)],
            -1,
            font,
        );
        let (check_w, row) = (px(CHECKBOX_W), px(24));
        for (i, label) in LABELS.iter().enumerate() {
            let id = FIRST_CHECKBOX + i as i32;
            let y = px(40) + row * i as i32;
            let style = WS_TABSTOP | BS_AUTOCHECKBOX as u32;
            let check = child(
                hwnd,
                "BUTTON",
                label,
                style,
                [(cw - check_w) / 2, y, check_w, px(20)],
                id,
                font,
            );
            SendMessageW(check, BM_SETCHECK, usize::from(initial[i]), 0);
        }
        let (button_w, gap, y, button_h) = (px(BUTTON_W), px(BUTTON_GAP), px(146), px(26));
        let x = (cw - 2 * button_w - gap) / 2;
        let ok = WS_TABSTOP | BS_DEFPUSHBUTTON as u32;
        child(hwnd, "BUTTON", "OK", ok, [x, y, button_w, button_h], IDOK, font);
        let cancel = WS_TABSTOP | BS_PUSHBUTTON as u32;
        child(
            hwnd,
            "BUTTON",
            "Cancel",
            cancel,
            [x + button_w + gap, y, button_w, button_h],
            IDCANCEL,
            font,
        );

        if !owner.is_null() {
            EnableWindow(owner, 0);
        }
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        SetForegroundWindow(hwnd);
        SetFocus(GetDlgItem(hwnd, FIRST_CHECKBOX));

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            // Tab, Enter and Esc are handled here, which sends IDOK and IDCANCEL to `wnd_proc`.
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        if !owner.is_null() {
            EnableWindow(owner, 1);
            SetForegroundWindow(owner);
        }
        UnregisterClassW(class.as_ptr(), instance);
        if !message_font.is_null() {
            DeleteObject(message_font);
        }
    }
    let bits = RESULT.load(Ordering::Relaxed);
    (bits & ACCEPTED != 0).then(|| std::array::from_fn(|i| bits >> i & 1 != 0))
}

/// `true` if another screensaver instance is already running in this session. The first caller
/// holds the named mutex until the process exits.
pub fn already_running() -> bool {
    let name = wide("Local\\OpenScranticSaver");
    // SAFETY: `name` is NUL-terminated; the handle is deliberately never closed.
    unsafe {
        let mutex = CreateMutexW(null(), 0, name.as_ptr());
        !mutex.is_null() && GetLastError() == ERROR_ALREADY_EXISTS
    }
}

/// Attributes of a borderless child window that fills `parent`, the preview pane of the Screen Saver
/// Settings window (the handle Windows passes after `/p`).
pub fn preview_attributes(parent: isize) -> WindowAttributes {
    // A handle that is already gone must not become the parent of a new window.
    let parent = if window_exists(parent) { parent } else { 0 };
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 1,
        bottom: 1,
    };
    // SAFETY: `rect` is valid for the call; a stale handle only makes it fail and leave the 1x1 default.
    unsafe {
        GetClientRect(parent as usize as HWND, &mut rect);
    }
    let size = PhysicalSize::new(
        (rect.right - rect.left).max(1) as u32,
        (rect.bottom - rect.top).max(1) as u32,
    );
    let attrs = Window::default_attributes()
        .with_title("Open Scrantic")
        .with_decorations(false)
        .with_inner_size(size)
        .with_position(PhysicalPosition::new(0, 0));
    match NonZeroIsize::new(parent) {
        // SAFETY: `parent` is the window Windows created for the preview and keeps alive while it runs.
        Some(handle) => unsafe {
            attrs.with_parent_window(Some(RawWindowHandle::Win32(Win32WindowHandle::new(handle))))
        },
        None => attrs,
    }
}

/// `false` once the window with this handle is gone: the preview ends with its pane.
pub fn window_exists(handle: isize) -> bool {
    // SAFETY: `IsWindow` accepts any handle value.
    unsafe { IsWindow(handle as usize as HWND) != 0 }
}

/// Shows a fatal error. The screensaver build has no console, so a message box is the only place for it.
pub fn error_box(text: &str) {
    let (text, title) = (wide(text), wide("Open Scrantic"));
    // SAFETY: both strings are NUL-terminated and outlive the call.
    unsafe {
        MessageBoxW(
            null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_scale_with_the_dpi_and_round_to_the_nearest_pixel() {
        assert_eq!(scaled(280, 96), 280);
        assert_eq!(scaled(280, 144), 420);
        assert_eq!(scaled(188, 120), 235);
        assert_eq!(scaled(1, 120), 1); // 1.25
        assert_eq!(scaled(3, 120), 4); // 3.75
        assert_eq!(scaled(0, 192), 0);
    }
}
