use std::mem::size_of;

use purepic::ui::layout::TITLE_BAR_HEIGHT_DIP;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
};
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTCLOSE, HTLEFT,
    HTMAXBUTTON, HTMINBUTTON, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, IsZoomed, SM_CXFRAME,
    SM_CXPADDEDBORDER, SM_CYFRAME,
};
use windows::core::{BOOL, Result};

const CAPTION_BUTTON_WIDTH_DIP: i32 = 46;

pub fn apply_dwm_attributes(hwnd: HWND) -> Result<()> {
    let dark_mode = BOOL(1);
    let corner = DWMWCP_ROUND;
    let margins = MARGINS {
        cxLeftWidth: 0,
        cxRightWidth: 0,
        cyTopHeight: 1,
        cyBottomHeight: 0,
    };

    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark_mode as *const _ as _,
            size_of::<BOOL>() as u32,
        )?;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as _,
            size_of_val(&corner) as u32,
        )?;
        DwmExtendFrameIntoClientArea(hwnd, &margins)?;
    }

    Ok(())
}

pub fn non_client_hit_test(hwnd: HWND, lparam: LPARAM) -> u32 {
    let mut window = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut window) }.is_err() {
        return HTCLIENT;
    }

    let screen_x = low_word_signed(lparam.0);
    let screen_y = high_word_signed(lparam.0);
    let x = screen_x - window.left;
    let y = screen_y - window.top;
    let width = window.right - window.left;
    let height = window.bottom - window.top;
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(1);
    let maximized = unsafe { IsZoomed(hwnd) }.as_bool();

    if !maximized {
        let frame_x = unsafe { GetSystemMetricsForDpi(SM_CXFRAME, dpi) }
            + unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
        let frame_y = unsafe { GetSystemMetricsForDpi(SM_CYFRAME, dpi) }
            + unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
        let left = x >= 0 && x < frame_x;
        let right = x < width && x >= width - frame_x;
        let top = y >= 0 && y < frame_y;
        let bottom = y < height && y >= height - frame_y;

        match (left, right, top, bottom) {
            (true, _, true, _) => return HTTOPLEFT,
            (_, true, true, _) => return HTTOPRIGHT,
            (true, _, _, true) => return HTBOTTOMLEFT,
            (_, true, _, true) => return HTBOTTOMRIGHT,
            (true, _, _, _) => return HTLEFT,
            (_, true, _, _) => return HTRIGHT,
            (_, _, true, _) => return HTTOP,
            (_, _, _, true) => return HTBOTTOM,
            _ => {}
        }
    }

    let title_height = scale_dip(TITLE_BAR_HEIGHT_DIP as i32, dpi);
    if y < 0 || y >= title_height {
        return HTCLIENT;
    }

    let button_width = scale_dip(CAPTION_BUTTON_WIDTH_DIP, dpi);
    if x >= width - button_width {
        HTCLOSE
    } else if x >= width - button_width * 2 {
        HTMAXBUTTON
    } else if x >= width - button_width * 3 {
        HTMINBUTTON
    } else {
        HTCAPTION
    }
}

fn scale_dip(value: i32, dpi: u32) -> i32 {
    ((value as i64 * dpi as i64 + 48) / 96) as i32
}

fn low_word_signed(value: isize) -> i32 {
    (value as u16 as i16) as i32
}

fn high_word_signed(value: isize) -> i32 {
    ((value as usize >> 16) as u16 as i16) as i32
}

#[cfg(test)]
mod tests {
    use super::scale_dip;

    #[test]
    fn scales_dip_to_physical_pixels() {
        assert_eq!(scale_dip(48, 96), 48);
        assert_eq!(scale_dip(48, 144), 72);
        assert_eq!(scale_dip(48, 192), 96);
    }
}
