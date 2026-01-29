//! macOS-style notification banners and dialogs for Windows
//!
//! Creates clean, modern UI that mimics macOS notification center banners
//! and centered dialogs with Segoe UI typography and accent colors.

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

// ─── Constants ───────────────────────────────────────────────────────────────

const BANNER_WIDTH: i32 = 420;
const BANNER_HEIGHT: i32 = 110;
const BANNER_TIMER_ID: usize = 1;
const BANNER_DISMISS_MS: u32 = 4000;

const DIALOG_BTN_W: i32 = 100;
const DIALOG_BTN_H: i32 = 36;
const DIALOG_PADDING: i32 = 24;

const BG_COLOR: u32 = 0x00FFFFFF;      // White background
const TITLE_COLOR: u32 = 0x00000000;   // Black title
const MSG_COLOR: u32 = 0x00555555;     // Gray message text
const ACCENT_GREEN: u32 = 0x0050C800;  // #00C850 (BGR)
const ACCENT_ORANGE: u32 = 0x0000A5FF; // #FFA500 (BGR)
const ACCENT_BLUE: u32 = 0x00FF9020;   // #2090FF (BGR)
const BTN_BG: u32 = 0x00F0F0F0;       // Light gray button
const BTN_BG_HOVER: u32 = 0x00E0E0E0; // Hover button

// ─── Thread-local storage for dialog data ────────────────────────────────────

struct BannerData {
    title: Vec<u16>,
    message: Vec<u16>,
    accent: u32,
}

struct DialogData {
    title: Vec<u16>,
    message: Vec<u16>,
    btn_hover: bool,
}

static REGISTERED_BANNER: AtomicBool = AtomicBool::new(false);
static REGISTERED_DIALOG: AtomicBool = AtomicBool::new(false);

fn set_data<T>(hwnd: HWND, data: *mut T) {
    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, data as isize); }
}

fn get_data<T>(hwnd: HWND) -> *mut T {
    unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut T }
}

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

fn wide_ptr(s: &[u16]) -> PCWSTR {
    PCWSTR(s.as_ptr())
}

// ─── Enable rounded corners on Windows 11 ────────────────────────────────────

fn enable_rounded_corners(hwnd: HWND) {
    use windows::Win32::Graphics::Dwm::*;
    // DWMWA_WINDOW_CORNER_PREFERENCE = 33, DWMWCP_ROUND = 2
    let preference: u32 = 2; // DWMWCP_ROUND
    let _ = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWINDOWATTRIBUTE(33),
            &preference as *const u32 as *const _,
            std::mem::size_of::<u32>() as u32,
        )
    };
}

fn make_font(height: i32, weight: i32) -> HFONT {
    let name = to_wide("Segoe UI");
    unsafe {
        CreateFontW(
            height, 0, 0, 0, weight, 0, 0, 0, 0, 0, 0, 0, 0,
            wide_ptr(&name),
        )
    }
}

// ─── Banner notification (auto-dismiss) ──────────────────────────────────────

/// Show a macOS-style notification banner at the top-center of the screen.
/// Auto-dismisses after 4 seconds.
pub fn show_banner(title: &str, message: &str, freed_mb: Option<f64>) {
    let accent = match freed_mb {
        Some(f) if f > 100.0 => ACCENT_GREEN,
        Some(f) if f > 0.0 => ACCENT_ORANGE,
        Some(_) => ACCENT_BLUE,
        None => ACCENT_BLUE,
    };

    let class_name = to_wide("RuVectorBanner");

    if !REGISTERED_BANNER.swap(true, Ordering::SeqCst) {
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(banner_wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: HINSTANCE(ptr::null_mut()),
            hIcon: HICON(ptr::null_mut()),
            hCursor: unsafe { LoadCursorW(HINSTANCE(ptr::null_mut()), IDC_ARROW).unwrap_or(HCURSOR(ptr::null_mut())) },
            hbrBackground: HBRUSH(ptr::null_mut()),
            lpszMenuName: wide_ptr(&to_wide("")),
            lpszClassName: wide_ptr(&class_name),
            hIconSm: HICON(ptr::null_mut()),
        };
        unsafe { RegisterClassExW(&wc); }
    }

    let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let x = screen_w - BANNER_WIDTH - 20; // Top-right corner with margin
    let y = 40;

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            wide_ptr(&class_name),
            wide_ptr(&to_wide("")),
            WS_POPUP | WS_VISIBLE,
            x, y, BANNER_WIDTH, BANNER_HEIGHT,
            HWND(ptr::null_mut()),
            HMENU(ptr::null_mut()),
            HINSTANCE(ptr::null_mut()),
            Some(ptr::null()),
        )
    };

    let hwnd = match hwnd {
        Ok(h) => h,
        Err(_) => return,
    };

    enable_rounded_corners(hwnd);

    let data = Box::new(BannerData {
        title: to_wide(title),
        message: to_wide(message),
        accent,
    });
    set_data(hwnd, Box::into_raw(data));

    unsafe {
        let _ = SetTimer(hwnd, BANNER_TIMER_ID, BANNER_DISMISS_MS, None);
    }

    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND(ptr::null_mut()), 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn banner_wndproc(
    hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let data = get_data::<BannerData>(hwnd);
            if data.is_null() {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            let data = &*data;

            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);

            // White background
            let bg_brush = CreateSolidBrush(COLORREF(BG_COLOR));
            FillRect(hdc, &rect, bg_brush);
            let _ = DeleteObject(bg_brush);

            // Left accent bar (6px)
            let accent_rect = RECT { left: 0, top: 0, right: 6, bottom: rect.bottom };
            let accent_brush = CreateSolidBrush(COLORREF(data.accent));
            FillRect(hdc, &accent_rect, accent_brush);
            let _ = DeleteObject(accent_brush);

            // Bottom border
            let border_rect = RECT { left: 0, top: rect.bottom - 1, right: rect.right, bottom: rect.bottom };
            let border_brush = CreateSolidBrush(COLORREF(0x00E0E0E0));
            FillRect(hdc, &border_rect, border_brush);
            let _ = DeleteObject(border_brush);

            SetBkMode(hdc, TRANSPARENT);

            // Title (Segoe UI Semibold 18px)
            let title_font = make_font(-22, 600);
            let old_font = SelectObject(hdc, title_font);
            SetTextColor(hdc, COLORREF(TITLE_COLOR));
            let mut title_rect = RECT { left: 20, top: 14, right: rect.right - 16, bottom: 44 };
            DrawTextW(hdc, &mut data.title.clone(), &mut title_rect,
                DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX);

            // Message (Segoe UI Regular 15px)
            let msg_font = make_font(-18, 400);
            SelectObject(hdc, msg_font);
            SetTextColor(hdc, COLORREF(MSG_COLOR));
            let mut msg_rect = RECT { left: 20, top: 48, right: rect.right - 16, bottom: rect.bottom - 10 };
            DrawTextW(hdc, &mut data.message.clone(), &mut msg_rect,
                DT_LEFT | DT_WORDBREAK | DT_END_ELLIPSIS | DT_NOPREFIX);

            // App name (gray, right-aligned)
            let app_font = make_font(-13, 400);
            SelectObject(hdc, app_font);
            SetTextColor(hdc, COLORREF(0x00999999));
            let mut app_text = to_wide("RuVector MemOpt");
            let mut app_rect = RECT { left: rect.right - 160, top: 14, right: rect.right - 12, bottom: 36 };
            DrawTextW(hdc, &mut app_text, &mut app_rect,
                DT_RIGHT | DT_SINGLELINE | DT_NOPREFIX);

            SelectObject(hdc, old_font);
            let _ = DeleteObject(title_font);
            let _ = DeleteObject(msg_font);
            let _ = DeleteObject(app_font);

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == BANNER_TIMER_ID {
                let _ = KillTimer(hwnd, BANNER_TIMER_ID);
                let data = get_data::<BannerData>(hwnd);
                if !data.is_null() {
                    let _ = Box::from_raw(data);
                }
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let _ = KillTimer(hwnd, BANNER_TIMER_ID);
            let data = get_data::<BannerData>(hwnd);
            if !data.is_null() {
                let _ = Box::from_raw(data);
            }
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ─── Info dialog (macOS-style centered dialog) ───────────────────────────────

/// Show a macOS-style centered dialog with OK button.
pub fn show_dialog(title: &str, message: &str) {
    let class_name = to_wide("RuVectorDialog");

    if !REGISTERED_DIALOG.swap(true, Ordering::SeqCst) {
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(dialog_wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: HINSTANCE(ptr::null_mut()),
            hIcon: HICON(ptr::null_mut()),
            hCursor: unsafe { LoadCursorW(HINSTANCE(ptr::null_mut()), IDC_ARROW).unwrap_or(HCURSOR(ptr::null_mut())) },
            hbrBackground: HBRUSH(ptr::null_mut()),
            lpszMenuName: wide_ptr(&to_wide("")),
            lpszClassName: wide_ptr(&class_name),
            hIconSm: HICON(ptr::null_mut()),
        };
        unsafe { RegisterClassExW(&wc); }
    }

    let line_count = message.lines().count().max(3);
    let text_height = (line_count as i32) * 22 + 24;
    let dialog_height = (220 + text_height).min(700);
    let dialog_width = 500;

    let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    let x = (screen_w - dialog_width) / 2;
    let y = (screen_h - dialog_height) / 2;

    let wtitle = to_wide(title);
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_DLGMODALFRAME,
            wide_ptr(&class_name),
            wide_ptr(&wtitle),
            WS_POPUP | WS_VISIBLE | WS_CAPTION,
            x, y, dialog_width, dialog_height,
            HWND(ptr::null_mut()),
            HMENU(ptr::null_mut()),
            HINSTANCE(ptr::null_mut()),
            Some(ptr::null()),
        )
    };

    let hwnd = match hwnd {
        Ok(h) => h,
        Err(_) => return,
    };

    enable_rounded_corners(hwnd);

    let data = Box::new(DialogData {
        title: to_wide(title),
        message: to_wide(message),
        btn_hover: false,
    });
    set_data(hwnd, Box::into_raw(data));

    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND(ptr::null_mut()), 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn get_ok_button_rect(client_rect: &RECT) -> RECT {
    RECT {
        left: (client_rect.right - DIALOG_BTN_W) / 2,
        top: client_rect.bottom - DIALOG_PADDING - DIALOG_BTN_H,
        right: (client_rect.right + DIALOG_BTN_W) / 2,
        bottom: client_rect.bottom - DIALOG_PADDING,
    }
}

fn point_in_rect(x: i32, y: i32, r: &RECT) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

unsafe extern "system" fn dialog_wndproc(
    hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let data = get_data::<DialogData>(hwnd);
            if data.is_null() {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            let data = &*data;

            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);

            // White background
            let bg_brush = CreateSolidBrush(COLORREF(BG_COLOR));
            FillRect(hdc, &rect, bg_brush);
            let _ = DeleteObject(bg_brush);

            // Top accent bar (3px, blue)
            let accent_rect = RECT { left: 0, top: 0, right: rect.right, bottom: 3 };
            let accent_brush = CreateSolidBrush(COLORREF(ACCENT_BLUE));
            FillRect(hdc, &accent_rect, accent_brush);
            let _ = DeleteObject(accent_brush);

            SetBkMode(hdc, TRANSPARENT);

            // Title (Segoe UI Semibold 20px)
            let title_font = make_font(-24, 600);
            let old_font = SelectObject(hdc, title_font);
            SetTextColor(hdc, COLORREF(TITLE_COLOR));
            let mut title_rect = RECT {
                left: DIALOG_PADDING,
                top: DIALOG_PADDING,
                right: rect.right - DIALOG_PADDING,
                bottom: DIALOG_PADDING + 30,
            };
            DrawTextW(hdc, &mut data.title.clone(), &mut title_rect,
                DT_CENTER | DT_SINGLELINE | DT_NOPREFIX);

            // Separator line
            let sep_y = DIALOG_PADDING + 38;
            let sep_rect = RECT { left: DIALOG_PADDING, top: sep_y, right: rect.right - DIALOG_PADDING, bottom: sep_y + 1 };
            let sep_brush = CreateSolidBrush(COLORREF(0x00E8E8E8));
            FillRect(hdc, &sep_rect, sep_brush);
            let _ = DeleteObject(sep_brush);

            // Message (Segoe UI 15px)
            let msg_font = make_font(-18, 400);
            SelectObject(hdc, msg_font);
            SetTextColor(hdc, COLORREF(MSG_COLOR));
            let mut msg_rect = RECT {
                left: DIALOG_PADDING,
                top: sep_y + 14,
                right: rect.right - DIALOG_PADDING,
                bottom: rect.bottom - DIALOG_PADDING - DIALOG_BTN_H - 16,
            };
            DrawTextW(hdc, &mut data.message.clone(), &mut msg_rect,
                DT_LEFT | DT_WORDBREAK | DT_NOPREFIX);

            // OK button
            let btn_rect = get_ok_button_rect(&rect);
            let btn_color = if data.btn_hover { BTN_BG_HOVER } else { BTN_BG };
            let btn_brush = CreateSolidBrush(COLORREF(btn_color));
            FillRect(hdc, &btn_rect, btn_brush);
            let _ = DeleteObject(btn_brush);

            // Button border
            let btn_border_brush = CreateSolidBrush(COLORREF(0x00CCCCCC));
            FrameRect(hdc, &btn_rect, btn_border_brush);
            let _ = DeleteObject(btn_border_brush);

            // Button text
            let btn_font = make_font(-16, 600);
            SelectObject(hdc, btn_font);
            SetTextColor(hdc, COLORREF(TITLE_COLOR));
            let mut btn_text = to_wide("OK");
            let mut btn_text_rect = btn_rect;
            DrawTextW(hdc, &mut btn_text, &mut btn_text_rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);

            SelectObject(hdc, old_font);
            let _ = DeleteObject(title_font);
            let _ = DeleteObject(msg_font);
            let _ = DeleteObject(btn_font);

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let data = get_data::<DialogData>(hwnd);
            if !data.is_null() {
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                let mut rect = RECT::default();
                let _ = GetClientRect(hwnd, &mut rect);
                let btn_rect = get_ok_button_rect(&rect);
                let hover = point_in_rect(x, y, &btn_rect);
                let data = &mut *data;
                if data.btn_hover != hover {
                    data.btn_hover = hover;
                    let _ = InvalidateRect(hwnd, None, TRUE);
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);
            let btn_rect = get_ok_button_rect(&rect);
            if point_in_rect(x, y, &btn_rect) {
                let data = get_data::<DialogData>(hwnd);
                if !data.is_null() {
                    let _ = Box::from_raw(data);
                }
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let vk = wparam.0 as u32;
            if vk == 0x0D || vk == 0x1B { // VK_RETURN or VK_ESCAPE
                let data = get_data::<DialogData>(hwnd);
                if !data.is_null() {
                    let _ = Box::from_raw(data);
                }
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
