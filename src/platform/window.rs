use std::mem::size_of;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BLACK_BRUSH, BeginPaint, EndPaint, FillRect, GetStockObject, HBRUSH, PAINTSTRUCT, UpdateWindow,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW,
    DispatchMessageW, GetMessageW, IDC_ARROW, LoadCursorW, MSG, PostQuitMessage, RegisterClassExW,
    SW_SHOW, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, ShowWindow, TranslateMessage,
    WINDOW_EX_STYLE, WM_DESTROY, WM_DPICHANGED, WM_PAINT, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
    WS_VISIBLE,
};
use windows::core::{Error, Result, w};

const INITIAL_WIDTH: i32 = 1280;
const INITIAL_HEIGHT: i32 = 800;

pub fn run() -> Result<()> {
    // The manifest is authoritative. This call keeps development builds DPI-aware
    // if the executable is launched without its embedded manifest.
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };

    let module = unsafe { GetModuleHandleW(None)? };
    let instance = HINSTANCE(module.0);
    let class_name = w!("PurePic.MainWindow");

    let window_class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
        lpszClassName: class_name,
        ..Default::default()
    };

    if unsafe { RegisterClassExW(&window_class) } == 0 {
        return Err(Error::from_win32());
    }

    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("PurePic"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            INITIAL_WIDTH,
            INITIAL_HEIGHT,
            None,
            None,
            Some(instance),
            None,
        )?
    };

    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
    }

    message_loop()
}

fn message_loop() -> Result<()> {
    let mut message = MSG::default();

    loop {
        let status = unsafe { GetMessageW(&mut message, None, 0, 0) };
        match status.0 {
            -1 => return Err(Error::from_win32()),
            0 => return Ok(()),
            _ => unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            },
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            let dc = unsafe { BeginPaint(hwnd, &mut paint) };
            let stock_brush = unsafe { GetStockObject(BLACK_BRUSH) };
            let brush = HBRUSH(stock_brush.0);
            unsafe {
                FillRect(dc, &paint.rcPaint, brush);
                let _ = EndPaint(hwnd, &paint);
            }
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let suggested = unsafe { &*(lparam.0 as *const RECT) };
            let _ = unsafe {
                SetWindowPos(
                    hwnd,
                    None,
                    suggested.left,
                    suggested.top,
                    suggested.right - suggested.left,
                    suggested.bottom - suggested.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )
            };
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}
