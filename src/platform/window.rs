use std::mem::size_of;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use crate::image::{DecodedImage, create_demo_image, decode_preview};
use crate::platform::chrome::{apply_dwm_attributes, non_client_hit_test};
use crate::render::{PointerAction, Renderer};
use purepic::ui::chrome::CaptionButton;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetMonitorInfoW, InvalidateRect, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MonitorFromWindow, PAINTSTRUCT, UpdateWindow,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW,
    DispatchMessageW, GWL_STYLE, GWLP_USERDATA, GetClientRect, GetMessageW, GetWindowLongPtrW,
    GetWindowPlacement, HTCLOSE, HTMAXBUTTON, HTMINBUTTON, IDC_ARROW, IsZoomed, LoadCursorW,
    LoadIconW, MSG, PostMessageW, PostQuitMessage, RegisterClassExW, SC_CLOSE, SC_MAXIMIZE,
    SC_MINIMIZE, SC_RESTORE, SW_SHOW, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SetWindowLongPtrW, SetWindowPlacement, SetWindowPos, ShowWindow,
    TranslateMessage, WINDOW_EX_STYLE, WINDOWPLACEMENT, WM_APP, WM_DESTROY, WM_DPICHANGED,
    WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCALCSIZE,
    WM_NCDESTROY, WM_NCLBUTTONDOWN, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE, WM_PAINT, WM_SIZE,
    WM_SYSCOMMAND, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
};
use windows::core::{Error, PCWSTR, Result, w};

const INITIAL_WIDTH: i32 = 1280;
const INITIAL_HEIGHT: i32 = 800;
const WM_APP_IMAGE_READY: u32 = WM_APP + 1;

type ImageLoadResult = std::result::Result<DecodedImage, String>;

struct WindowState {
    renderer: Renderer,
    image_receiver: Receiver<ImageLoadResult>,
    requested_path: Option<PathBuf>,
    slider_dragging: bool,
    image_dragging: bool,
    fullscreen: bool,
    windowed_placement: Option<WINDOWPLACEMENT>,
}

pub fn run(image_path: Option<PathBuf>) -> Result<()> {
    // The manifest is authoritative. This call keeps development builds DPI-aware
    // if the executable is launched without its embedded manifest.
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };

    let module = unsafe { GetModuleHandleW(None)? };
    let instance = HINSTANCE(module.0);
    let class_name = w!("PurePic.MainWindow");
    let app_icon =
        unsafe { LoadIconW(Some(instance), PCWSTR(1_usize as *const u16)) }.unwrap_or_default();

    let window_class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        hIcon: app_icon,
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
        hIconSm: app_icon,
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
            WS_OVERLAPPEDWINDOW,
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

    apply_dwm_attributes(hwnd)?;
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        )?;
    }
    let mut client = RECT::default();
    unsafe { GetClientRect(hwnd, &mut client)? };
    let (image_sender, image_receiver) = channel();
    let mut renderer = Renderer::new(
        hwnd,
        unsafe { GetDpiForWindow(hwnd) },
        (client.right - client.left).max(0) as u32,
        (client.bottom - client.top).max(0) as u32,
    )?;
    if let Some(path) = &image_path {
        renderer.set_loading(path);
    } else {
        renderer.set_image(create_demo_image())?;
    }
    let state = Box::new(WindowState {
        renderer,
        image_receiver,
        requested_path: image_path.clone(),
        slider_dragging: false,
        image_dragging: false,
        fullscreen: false,
        windowed_placement: None,
    });
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
    }

    if let Some(path) = image_path {
        spawn_image_decode(
            hwnd,
            path,
            (client.right - client.left).max(1) as u32,
            (client.bottom - client.top).max(1) as u32,
            image_sender,
        );
    }

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
            unsafe { BeginPaint(hwnd, &mut paint) };
            if let Some(state) = unsafe { state_mut(hwnd) } {
                let _ = state.renderer.render();
            }
            unsafe {
                let _ = EndPaint(hwnd, &paint);
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_LBUTTONDOWN => {
            let mut fullscreen_request = None;
            if let Some(state) = unsafe { state_mut(hwnd) } {
                let x = signed_low_word(lparam.0);
                let y = signed_high_word(lparam.0);
                match state.renderer.pointer_down(x, y) {
                    PointerAction::BeginSlider => {
                        state.slider_dragging = true;
                        unsafe { SetCapture(hwnd) };
                    }
                    PointerAction::BeginPan => {
                        state.image_dragging = true;
                        unsafe { SetCapture(hwnd) };
                    }
                    PointerAction::ToggleFullscreen => {
                        fullscreen_request = Some(!state.fullscreen);
                    }
                    PointerAction::None => {}
                }
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
            }
            if let Some(fullscreen) = fullscreen_request {
                let _ = unsafe { set_fullscreen(hwnd, fullscreen) };
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if let Some(state) = unsafe { state_mut(hwnd) } {
                let status_changed = state
                    .renderer
                    .set_status_hot(signed_low_word(lparam.0), signed_high_word(lparam.0));
                if state.slider_dragging {
                    state
                        .renderer
                        .pointer_move_slider(signed_low_word(lparam.0));
                } else if state.image_dragging {
                    state
                        .renderer
                        .pointer_move_pan(signed_low_word(lparam.0), signed_high_word(lparam.0));
                } else if !status_changed {
                    return LRESULT(0);
                }
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
            }
            let mut tracking = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                ..Default::default()
            };
            let _ = unsafe { TrackMouseEvent(&mut tracking) };
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            if let Some(state) = unsafe { state_mut(hwnd) }
                && state.renderer.clear_status_hot()
            {
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            if let Some(state) = unsafe { state_mut(hwnd) }
                && (state.slider_dragging || state.image_dragging)
            {
                state.slider_dragging = false;
                state.image_dragging = false;
                state.renderer.end_pan();
                let _ = unsafe { ReleaseCapture() };
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let mut fullscreen_request = None;
            if let Some(state) = unsafe { state_mut(hwnd) } {
                match wparam.0 as u32 {
                    0x7A => {
                        fullscreen_request = Some(!state.fullscreen);
                    }
                    0x1B if state.fullscreen => {
                        fullscreen_request = Some(false);
                    }
                    0x1B => {
                        if state.renderer.close_zoom_menu() {
                            let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                        }
                    }
                    _ => {}
                }
            }
            if let Some(fullscreen) = fullscreen_request {
                let _ = unsafe { set_fullscreen(hwnd, fullscreen) };
            }
            LRESULT(0)
        }
        WM_NCCALCSIZE => {
            if wparam.0 != 0 {
                return LRESULT(0);
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        windows::Win32::UI::WindowsAndMessaging::WM_NCHITTEST => {
            LRESULT(non_client_hit_test(hwnd, lparam) as isize)
        }
        WM_NCLBUTTONDOWN => {
            let command = match wparam.0 as u32 {
                HTMINBUTTON => Some(SC_MINIMIZE),
                HTMAXBUTTON if unsafe { IsZoomed(hwnd) }.as_bool() => Some(SC_RESTORE),
                HTMAXBUTTON => Some(SC_MAXIMIZE),
                HTCLOSE => Some(SC_CLOSE),
                _ => None,
            };
            if let Some(command) = command {
                let _ = unsafe {
                    PostMessageW(
                        Some(hwnd),
                        WM_SYSCOMMAND,
                        WPARAM(command as usize),
                        LPARAM(0),
                    )
                };
                LRESULT(0)
            } else {
                unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
            }
        }
        WM_NCMOUSEMOVE => {
            if let Some(state) = unsafe { state_mut(hwnd) } {
                state
                    .renderer
                    .set_caption_hot(caption_button_from_hit(wparam.0 as u32));
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
            }
            let mut tracking = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE | TME_NONCLIENT,
                hwndTrack: hwnd,
                ..Default::default()
            };
            let _ = unsafe { TrackMouseEvent(&mut tracking) };
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_NCMOUSELEAVE => {
            if let Some(state) = unsafe { state_mut(hwnd) } {
                state.renderer.set_caption_hot(CaptionButton::None);
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
            }
            LRESULT(0)
        }
        WM_APP_IMAGE_READY => {
            if let Some(state) = unsafe { state_mut(hwnd) } {
                while let Ok(result) = state.image_receiver.try_recv() {
                    match result {
                        Ok(image) => {
                            if let Err(error) = state.renderer.set_image(image)
                                && let Some(path) = &state.requested_path
                            {
                                state.renderer.set_image_error(path, &error.to_string());
                            }
                        }
                        Err(error) => {
                            if let Some(path) = &state.requested_path {
                                state.renderer.set_image_error(path, &error);
                            }
                        }
                    }
                }
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
            }
            LRESULT(0)
        }
        WM_SIZE => {
            let width = (lparam.0 as u32 & 0xFFFF) as u32;
            let height = ((lparam.0 as u32 >> 16) & 0xFFFF) as u32;
            if let Some(state) = unsafe { state_mut(hwnd) } {
                state
                    .renderer
                    .set_maximized(unsafe { IsZoomed(hwnd) }.as_bool());
                let _ = state.renderer.resize(width, height);
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
            if let Some(state) = unsafe { state_mut(hwnd) } {
                state.renderer.set_dpi(unsafe { GetDpiForWindow(hwnd) });
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        WM_NCDESTROY => {
            let pointer = unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) } as *mut WindowState;
            if !pointer.is_null() {
                drop(unsafe { Box::from_raw(pointer) });
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

unsafe fn state_mut(hwnd: HWND) -> Option<&'static mut WindowState> {
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut WindowState;
    unsafe { pointer.as_mut() }
}

fn caption_button_from_hit(hit: u32) -> CaptionButton {
    match hit {
        HTMINBUTTON => CaptionButton::Minimize,
        HTMAXBUTTON => CaptionButton::Maximize,
        HTCLOSE => CaptionButton::Close,
        _ => CaptionButton::None,
    }
}

fn signed_low_word(value: isize) -> i32 {
    value as u16 as i16 as i32
}

fn signed_high_word(value: isize) -> i32 {
    (value as u32 >> 16) as u16 as i16 as i32
}

unsafe fn set_fullscreen(hwnd: HWND, fullscreen: bool) -> Result<()> {
    let current = unsafe { state_mut(hwnd) }.is_some_and(|state| state.fullscreen);
    if current == fullscreen {
        return Ok(());
    }

    if fullscreen {
        let mut placement = WINDOWPLACEMENT {
            length: size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        unsafe { GetWindowPlacement(hwnd, &mut placement)? };
        let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
        let mut monitor_info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        unsafe { GetMonitorInfoW(monitor, &mut monitor_info).ok()? };
        let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32;
        if let Some(state) = unsafe { state_mut(hwnd) } {
            state.windowed_placement = Some(placement);
            state.fullscreen = true;
            state.renderer.set_fullscreen(true);
        }
        unsafe { SetWindowLongPtrW(hwnd, GWL_STYLE, (style & !WS_OVERLAPPEDWINDOW.0) as isize) };
        let rect = monitor_info.rcMonitor;
        unsafe {
            SetWindowPos(
                hwnd,
                None,
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_FRAMECHANGED | SWP_NOACTIVATE,
            )?
        };
    } else {
        let placement = if let Some(state) = unsafe { state_mut(hwnd) } {
            state.fullscreen = false;
            state.renderer.set_fullscreen(false);
            state.windowed_placement.take()
        } else {
            None
        };
        let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32;
        unsafe { SetWindowLongPtrW(hwnd, GWL_STYLE, (style | WS_OVERLAPPEDWINDOW.0) as isize) };
        if let Some(placement) = placement {
            unsafe { SetWindowPlacement(hwnd, &placement)? };
        }
        unsafe {
            SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )?
        };
    }
    let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
    Ok(())
}

fn spawn_image_decode(
    hwnd: HWND,
    path: PathBuf,
    target_width: u32,
    target_height: u32,
    sender: std::sync::mpsc::Sender<ImageLoadResult>,
) {
    let raw_hwnd = hwnd.0 as usize;
    std::thread::spawn(move || {
        let result =
            decode_preview(&path, target_width, target_height).map_err(|error| error.to_string());
        let _ = sender.send(result);
        let hwnd = HWND(raw_hwnd as *mut _);
        let _ = unsafe { PostMessageW(Some(hwnd), WM_APP_IMAGE_READY, WPARAM(0), LPARAM(0)) };
    });
}
