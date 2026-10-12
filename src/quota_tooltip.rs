//! Compact reset-time tooltips using the widget's colors and typography.
use std::sync::Mutex;

use windows::core::{w, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::{DeleteObject, HFONT};
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Default)]
struct State {
    window: isize,
    owner: isize,
    key: String,
    texts: Vec<Vec<u16>>,
    font: isize,
}

static STATE: Mutex<State> = Mutex::new(State {
    window: 0,
    owner: 0,
    key: String::new(),
    texts: Vec::new(),
    font: 0,
});

#[derive(Clone, Copy)]
pub struct Style {
    pub background: u32,
    pub text: u32,
    pub font_px: i32,
}

// The app has no comctl32 v6 manifest. Exclude the v6-only reserved field so
// the standard tooltip control accepts these TOOLINFO structures.
const TOOL_INFO_SIZE: u32 = std::mem::offset_of!(TTTOOLINFOW, lpReserved) as u32;

/// Called on the UI thread as displayed data and freshness change. Rects
/// are widget-client coordinates, so the tips also work when it is dragged.
pub fn sync(owner: HWND, regions: Vec<(RECT, String)>, style: Style) {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let region_key = regions
        .iter()
        .map(|(r, t)| format!("{},{},{},{}:{t}", r.left, r.top, r.right, r.bottom))
        .collect::<Vec<_>>()
        .join("|");
    let key = format!(
        "{},{},{}:{region_key}",
        style.background, style.text, style.font_px
    );
    if state.owner == owner.0 as isize && state.key == key {
        return;
    }
    unsafe {
        let old = HWND(state.window as *mut _);
        if state.window != 0 {
            for index in 0..state.texts.len() {
                let info = TTTOOLINFOW {
                    cbSize: TOOL_INFO_SIZE,
                    hwnd: HWND(state.owner as *mut _),
                    uId: index + 1,
                    ..Default::default()
                };
                SendMessageW(
                    old,
                    TTM_DELTOOLW,
                    WPARAM(0),
                    LPARAM(&info as *const _ as isize),
                );
            }
            if state.owner != owner.0 as isize {
                let _ = DestroyWindow(old);
                state.window = 0;
            }
        }
        state.owner = owner.0 as isize;
        state.key = key;
        state.texts.clear();
        if regions.is_empty() {
            return;
        }
        if state.window == 0 {
            let controls = INITCOMMONCONTROLSEX {
                dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_WIN95_CLASSES,
            };
            let _ = InitCommonControlsEx(&controls);
            let Ok(window) = CreateWindowExW(
                WS_EX_TOPMOST,
                w!("tooltips_class32"),
                w!(""),
                WS_POPUP | WINDOW_STYLE(TTS_ALWAYSTIP | TTS_NOPREFIX),
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                owner,
                None,
                None,
                None,
            ) else {
                state.key.clear();
                return;
            };
            state.window = window.0 as isize;
            SendMessageW(window, TTM_SETMAXTIPWIDTH, WPARAM(0), LPARAM(-1));
        }
        let window = HWND(state.window as *mut _);
        // Disable the generic system tooltip palette so our colors apply.
        let _ = SetWindowTheme(window, w!(""), w!(""));
        SendMessageW(
            window,
            TTM_SETTIPBKCOLOR,
            WPARAM(style.background as usize),
            LPARAM(0),
        );
        SendMessageW(
            window,
            TTM_SETTIPTEXTCOLOR,
            WPARAM(style.text as usize),
            LPARAM(0),
        );
        let margin = RECT {
            left: 6,
            top: 4,
            right: 6,
            bottom: 4,
        };
        SendMessageW(
            window,
            TTM_SETMARGIN,
            WPARAM(0),
            LPARAM(&margin as *const _ as isize),
        );
        let font = crate::quota_text::create_reset_font(style.font_px);
        if !font.is_invalid() {
            SendMessageW(window, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
            if state.font != 0 {
                let _ = DeleteObject(HFONT(state.font as *mut _));
            }
            state.font = font.0 as isize;
        }
        for (index, (rect, text)) in regions.into_iter().enumerate() {
            let mut wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
            let info = TTTOOLINFOW {
                cbSize: TOOL_INFO_SIZE,
                uFlags: TTF_SUBCLASS,
                hwnd: owner,
                uId: index + 1,
                rect,
                lpszText: PWSTR(wide.as_mut_ptr()),
                ..Default::default()
            };
            SendMessageW(
                HWND(state.window as *mut _),
                TTM_ADDTOOLW,
                WPARAM(0),
                LPARAM(&info as *const _ as isize),
            );
            state.texts.push(wide);
        }
    }
}

pub fn clear() {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if state.window != 0 {
        unsafe {
            let _ = DestroyWindow(HWND(state.window as *mut _));
        }
    }
    if state.font != 0 {
        unsafe {
            let _ = DeleteObject(HFONT(state.font as *mut _));
        }
    }
    *state = State::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_tooltips_follow_available_regions() {
        unsafe {
            let owner = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("quota-tip-test"),
                WS_POPUP,
                0,
                0,
                600,
                60,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            let rect = RECT {
                left: 200,
                top: 0,
                right: 400,
                bottom: 20,
            };
            let style = Style {
                background: 0x1c1c1c,
                text: 0xd8d8d8,
                font_px: 12,
            };
            sync(
                owner,
                vec![
                    (rect, "Thu 10/15/26 8:52AM".into()),
                    (rect, "Mon 10/19/26 9:15PM".into()),
                ],
                style,
            );
            let tooltip = HWND(STATE.lock().unwrap().window as *mut _);
            assert_eq!(
                SendMessageW(tooltip, TTM_GETTOOLCOUNT, WPARAM(0), LPARAM(0)).0,
                2
            );
            assert_eq!(
                SendMessageW(tooltip, TTM_GETTIPBKCOLOR, WPARAM(0), LPARAM(0)).0 as u32,
                style.background
            );
            assert_eq!(
                SendMessageW(tooltip, TTM_GETTIPTEXTCOLOR, WPARAM(0), LPARAM(0)).0 as u32,
                style.text
            );
            assert_ne!(SendMessageW(tooltip, WM_GETFONT, WPARAM(0), LPARAM(0)).0, 0);
            sync(owner, vec![(rect, "Thu 10/15/26 8:52AM".into())], style);
            let mut buffer = [0u16; 256];
            let info = TTTOOLINFOW {
                cbSize: TOOL_INFO_SIZE,
                hwnd: owner,
                uId: 1,
                lpszText: PWSTR(buffer.as_mut_ptr()),
                ..Default::default()
            };
            SendMessageW(
                tooltip,
                TTM_GETTEXTW,
                WPARAM(buffer.len()),
                LPARAM(&info as *const _ as isize),
            );
            let length = buffer.iter().position(|&c| c == 0).unwrap();
            assert_eq!(
                String::from_utf16(&buffer[..length]).unwrap(),
                "Thu 10/15/26 8:52AM"
            );
            assert_eq!(
                SendMessageW(tooltip, TTM_GETTOOLCOUNT, WPARAM(0), LPARAM(0)).0,
                1
            );
            sync(owner, Vec::new(), style);
            assert_eq!(
                SendMessageW(tooltip, TTM_GETTOOLCOUNT, WPARAM(0), LPARAM(0)).0,
                0
            );
            clear();
            let _ = DestroyWindow(owner);
        }
    }
}
