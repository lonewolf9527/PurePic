use std::mem::size_of;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::image::{DecodedImage, decode_preview};
use crate::platform::chrome::{apply_dwm_attributes, non_client_hit_test};
use crate::platform::recycle::move_file_to_recycle_bin;
use crate::platform::registry::{self, SavedWindowState, ThumbnailPreferences};
use crate::platform::thumbnails::{
    DirectoryScanResult, ThumbnailLoader, ThumbnailTask, spawn_directory_scan,
};
use crate::render::{PointerAction, Renderer};
use purepic::ui::chrome::CaptionButton;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetMonitorInfoW, InvalidateRect, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MonitorFromWindow, PAINTSTRUCT, ScreenToClient, UpdateWindow,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, GetSystemMetricsForDpi,
    SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW,
    DispatchMessageW, GWL_STYLE, GWLP_USERDATA, GetClientRect, GetCursorPos, GetMessageW,
    GetWindowLongPtrW, GetWindowPlacement, HTCLIENT, HTCLOSE, HTMAXBUTTON, HTMINBUTTON, IDC_ARROW,
    IDC_HAND, IsZoomed, KillTimer, LoadCursorW, LoadIconW, MINMAXINFO, MSG, NCCALCSIZE_PARAMS,
    PostMessageW, PostQuitMessage, RegisterClassExW, SC_CLOSE, SC_MAXIMIZE, SC_MINIMIZE,
    SC_RESTORE, SM_CXFRAME, SM_CXPADDEDBORDER, SM_CYFRAME, SW_SHOW, SW_SHOWMAXIMIZED,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetCursor, SetTimer,
    SetWindowLongPtrW, SetWindowPlacement, SetWindowPos, ShowWindow, TranslateMessage,
    WINDOW_EX_STYLE, WINDOWPLACEMENT, WM_APP, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND,
    WM_GETMINMAXINFO, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL,
    WM_NCCALCSIZE, WM_NCDESTROY, WM_NCLBUTTONDOWN, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE, WM_PAINT,
    WM_SETCURSOR, WM_SIZE, WM_SYSCOMMAND, WM_TIMER, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
    WS_THICKFRAME,
};
use windows::core::{Error, PCWSTR, Result, w};

const INITIAL_WIDTH: i32 = 1920;
const INITIAL_HEIGHT: i32 = 1080;
const MINIMUM_WIDTH: i32 = 890;
const MINIMUM_HEIGHT: i32 = 890;
const WM_APP_IMAGE_READY: u32 = WM_APP + 1;
const WM_APP_DIRECTORY_READY: u32 = WM_APP + 2;
const WM_APP_THUMBNAIL_READY: u32 = WM_APP + 3;
const NAVIGATION_TIMER_ID: usize = 1;
const NAVIGATION_TIMER_INTERVAL_MS: u32 = 16;

struct ImageLoadResult {
    generation: u64,
    path: PathBuf,
    decoded: std::result::Result<DecodedImage, String>,
}

struct WindowState {
    renderer: Renderer,
    image_receiver: Receiver<ImageLoadResult>,
    image_sender: Sender<ImageLoadResult>,
    image_generation: u64,
    directory_receiver: Receiver<DirectoryScanResult>,
    directory_sender: Sender<DirectoryScanResult>,
    directory_generation: u64,
    directory_scan_started: bool,
    thumbnail_loader: ThumbnailLoader,
    requested_path: Option<PathBuf>,
    slider_dragging: bool,
    thumbnail_scroll_dragging: bool,
    image_dragging: bool,
    context_menu_registered: bool,
    fullscreen: bool,
    windowed_placement: Option<WINDOWPLACEMENT>,
}

pub fn run(image_path: Option<PathBuf>) -> Result<()> {
    // The manifest is authoritative. This call keeps development builds DPI-aware
    // if the executable is launched without its embedded manifest.
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };

    let saved_window = registry::load_window_state();
    let initial_width = saved_window
        .map(|state| state.width.clamp(MINIMUM_WIDTH as u32, u16::MAX as u32) as i32)
        .unwrap_or(INITIAL_WIDTH);
    let initial_height = saved_window
        .map(|state| state.height.clamp(MINIMUM_HEIGHT as u32, u16::MAX as u32) as i32)
        .unwrap_or(INITIAL_HEIGHT);
    let module = unsafe { GetModuleHandleW(None)? };
    let instance = HINSTANCE(module.0);
    let class_name = w!("PurePic.MainWindow");
    let app_icon = unsafe {
        LoadIconW(
            Some(instance),
            PCWSTR(std::ptr::with_exposed_provenance(1_usize)),
        )
    }
    .unwrap_or_default();

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
            initial_width,
            initial_height,
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
    let (directory_sender, directory_receiver) = channel();
    let thumbnail_loader = ThumbnailLoader::new(hwnd, WM_APP_THUMBNAIL_READY);
    let mut renderer = Renderer::new(
        hwnd,
        unsafe { GetDpiForWindow(hwnd) },
        (client.right - client.left).max(0) as u32,
        (client.bottom - client.top).max(0) as u32,
    )?;
    let thumbnail_preferences = registry::load_thumbnail_preferences();
    renderer.set_thumbnail_preferences(thumbnail_preferences.visible, thumbnail_preferences.dock);
    if let Some(path) = &image_path {
        renderer.set_loading(path);
    }
    let context_menu_registered = registry::is_context_menu_registered();
    renderer.set_context_menu_registered(context_menu_registered);
    let state = Box::new(WindowState {
        renderer,
        image_receiver,
        image_sender: image_sender.clone(),
        image_generation: 1,
        directory_receiver,
        directory_sender,
        directory_generation: 0,
        directory_scan_started: false,
        thumbnail_loader,
        requested_path: image_path.clone(),
        slider_dragging: false,
        thumbnail_scroll_dragging: false,
        image_dragging: false,
        context_menu_registered,
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
            1,
        );
    }

    unsafe {
        let show_command = if saved_window.is_some_and(|state| state.maximized) {
            SW_SHOWMAXIMIZED
        } else {
            SW_SHOW
        };
        let _ = ShowWindow(hwnd, show_command);
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
        WM_GETMINMAXINFO => {
            let bounds = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
            enforce_window_bounds(hwnd, bounds);
            LRESULT(0)
        }
        WM_SETCURSOR if (lparam.0 as u32 & 0xFFFF) == HTCLIENT => {
            if let Some(state) = unsafe { state_mut(hwnd) } {
                let mut point = POINT::default();
                if unsafe { GetCursorPos(&mut point) }.is_ok()
                    && unsafe { ScreenToClient(hwnd, &mut point) }.as_bool()
                    && !state.renderer.is_over_thumbnail_panel(point.x, point.y)
                    && (state.image_dragging || state.renderer.shows_pan_cursor(point.x, point.y))
                    && let Ok(cursor) = unsafe { LoadCursorW(None, IDC_HAND) }
                {
                    unsafe { SetCursor(Some(cursor)) };
                    return LRESULT(1);
                }
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
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
                    PointerAction::BeginThumbnailScroll => {
                        state.thumbnail_scroll_dragging = true;
                        unsafe { SetCapture(hwnd) };
                    }
                    PointerAction::BeginPan => {
                        state.image_dragging = true;
                        unsafe { SetCapture(hwnd) };
                    }
                    PointerAction::ToggleFullscreen => {
                        fullscreen_request = Some(!state.fullscreen);
                    }
                    PointerAction::DeleteCurrent => {
                        delete_current_image(hwnd, state);
                    }
                    PointerAction::ToggleContextMenu => {
                        let registered = !state.context_menu_registered;
                        if registry::set_context_menu_registered(registered).is_ok() {
                            state.context_menu_registered = registered;
                            state.renderer.set_context_menu_registered(registered);
                        }
                    }
                    PointerAction::OpenDefaultAppSettings => {
                        let _ = registry::register_default_app_and_open_settings();
                    }
                    PointerAction::OpenThumbnail(index) => {
                        open_thumbnail(hwnd, state, index);
                    }
                    PointerAction::ThumbnailPreferencesChanged => {
                        save_thumbnail_preferences(state);
                    }
                    PointerAction::None => {}
                }
                queue_thumbnail_requests(state);
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
            }
            if let Some(fullscreen) = fullscreen_request {
                let _ = unsafe { set_fullscreen(hwnd, fullscreen) };
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if let Some(state) = unsafe { state_mut(hwnd) } {
                let pointer_hot_changed = state
                    .renderer
                    .set_pointer_hot(signed_low_word(lparam.0), signed_high_word(lparam.0));
                update_navigation_timer(hwnd, state);
                if state.slider_dragging {
                    state
                        .renderer
                        .pointer_move_slider(signed_low_word(lparam.0));
                } else if state.thumbnail_scroll_dragging {
                    state.renderer.pointer_move_thumbnail_scroll(
                        signed_low_word(lparam.0),
                        signed_high_word(lparam.0),
                    );
                    queue_thumbnail_requests(state);
                } else if state.image_dragging {
                    state
                        .renderer
                        .pointer_move_pan(signed_low_word(lparam.0), signed_high_word(lparam.0));
                } else if !pointer_hot_changed {
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
        WM_MOUSEWHEEL => {
            if let Some(state) = unsafe { state_mut(hwnd) } {
                let mut point = POINT {
                    x: signed_low_word(lparam.0),
                    y: signed_high_word(lparam.0),
                };
                if unsafe { ScreenToClient(hwnd, &mut point) }.as_bool() {
                    let wheel_delta = signed_high_word(wparam.0 as isize) as i16;
                    if state
                        .renderer
                        .scroll_thumbnails(point.x, point.y, wheel_delta)
                    {
                        queue_thumbnail_requests(state);
                        let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                        return LRESULT(0);
                    }
                    if state.renderer.zoom_canvas_at(point.x, point.y, wheel_delta) {
                        let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                        return LRESULT(0);
                    }
                }
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_MOUSELEAVE => {
            if let Some(state) = unsafe { state_mut(hwnd) } {
                let changed = state.renderer.clear_pointer_hot();
                update_navigation_timer(hwnd, state);
                if changed {
                    let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            if let Some(state) = unsafe { state_mut(hwnd) }
                && (state.slider_dragging
                    || state.thumbnail_scroll_dragging
                    || state.image_dragging)
            {
                state.slider_dragging = false;
                state.thumbnail_scroll_dragging = false;
                state.image_dragging = false;
                state.renderer.end_thumbnail_scroll();
                state.renderer.end_pan();
                let _ = unsafe { ReleaseCapture() };
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let mut fullscreen_request = None;
            if let Some(state) = unsafe { state_mut(hwnd) } {
                match wparam.0 as u32 {
                    0x25 => {
                        if let Some(index) = state.renderer.adjacent_thumbnail_index(-1) {
                            open_thumbnail(hwnd, state, index);
                            queue_thumbnail_requests(state);
                            let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                        }
                    }
                    0x27 => {
                        if let Some(index) = state.renderer.adjacent_thumbnail_index(1) {
                            open_thumbnail(hwnd, state, index);
                            queue_thumbnail_requests(state);
                            let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                        }
                    }
                    0x2E => {
                        delete_current_image(hwnd, state);
                    }
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
        WM_TIMER if wparam.0 == NAVIGATION_TIMER_ID => {
            if let Some(state) = unsafe { state_mut(hwnd) } {
                let changed = state.renderer.tick_navigation_animation();
                update_navigation_timer(hwnd, state);
                if changed {
                    let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                }
            }
            LRESULT(0)
        }
        WM_NCCALCSIZE => {
            if wparam.0 != 0 {
                let parameters = unsafe { &mut *(lparam.0 as *mut NCCALCSIZE_PARAMS) };
                adjust_maximized_client_rect(hwnd, &mut parameters.rgrc[0]);
                return LRESULT(0);
            }
            let rect = unsafe { &mut *(lparam.0 as *mut RECT) };
            if adjust_maximized_client_rect(hwnd, rect) {
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
                    if result.generation != state.image_generation {
                        continue;
                    }
                    state.requested_path = Some(result.path.clone());
                    match result.decoded {
                        Ok(image) => {
                            if let Err(error) = state.renderer.set_image(image) {
                                state
                                    .renderer
                                    .set_image_error(&result.path, &error.to_string());
                            } else if !state.directory_scan_started {
                                state.directory_generation =
                                    state.directory_generation.wrapping_add(1);
                                state.directory_scan_started = true;
                                spawn_directory_scan(
                                    hwnd,
                                    result.path,
                                    state.directory_generation,
                                    state.directory_sender.clone(),
                                    WM_APP_DIRECTORY_READY,
                                );
                            }
                        }
                        Err(error) => {
                            state.renderer.set_image_error(&result.path, &error);
                        }
                    }
                }
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
            }
            LRESULT(0)
        }
        WM_APP_DIRECTORY_READY => {
            if let Some(state) = unsafe { state_mut(hwnd) } {
                while let Ok(result) = state.directory_receiver.try_recv() {
                    if result.generation != state.directory_generation {
                        continue;
                    }
                    if let Some(current_path) = state.requested_path.as_deref() {
                        state
                            .renderer
                            .set_thumbnail_catalog(result.paths, current_path);
                    }
                }
                queue_thumbnail_requests(state);
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
            }
            LRESULT(0)
        }
        WM_APP_THUMBNAIL_READY => {
            if let Some(state) = unsafe { state_mut(hwnd) } {
                while let Ok(result) = state.thumbnail_loader.results.try_recv() {
                    if result.generation != state.directory_generation {
                        continue;
                    }
                    match result.decoded {
                        Ok(image) => {
                            if state
                                .renderer
                                .set_thumbnail_image(
                                    result.index,
                                    &result.path,
                                    result.target_size_px,
                                    image,
                                )
                                .is_err()
                            {
                                state.renderer.set_thumbnail_failed(
                                    result.index,
                                    &result.path,
                                    result.target_size_px,
                                );
                            }
                        }
                        Err(_) => state.renderer.set_thumbnail_failed(
                            result.index,
                            &result.path,
                            result.target_size_px,
                        ),
                    }
                }
                queue_thumbnail_requests(state);
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
            }
            LRESULT(0)
        }
        WM_SIZE => {
            let width = lparam.0 as u32 & 0xFFFF;
            let height = (lparam.0 as u32 >> 16) & 0xFFFF;
            if let Some(state) = unsafe { state_mut(hwnd) } {
                state
                    .renderer
                    .set_maximized(unsafe { IsZoomed(hwnd) }.as_bool());
                let _ = state.renderer.resize(width, height);
                queue_thumbnail_requests(state);
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
                queue_thumbnail_requests(state);
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            let _ = unsafe { KillTimer(Some(hwnd), NAVIGATION_TIMER_ID) };
            if let Some(state) = unsafe { state_mut(hwnd) } {
                if let Some(saved) = saved_window_state(hwnd, state) {
                    let _ = registry::save_window_state(saved);
                }
                save_thumbnail_preferences(state);
            }
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
    generation: u64,
) {
    let raw_hwnd = hwnd.0 as usize;
    std::thread::spawn(move || {
        let decoded =
            decode_preview(&path, target_width, target_height).map_err(|error| error.to_string());
        let _ = sender.send(ImageLoadResult {
            generation,
            path,
            decoded,
        });
        let hwnd = HWND(raw_hwnd as *mut _);
        let _ = unsafe { PostMessageW(Some(hwnd), WM_APP_IMAGE_READY, WPARAM(0), LPARAM(0)) };
    });
}

fn request_image(hwnd: HWND, state: &mut WindowState, path: PathBuf) {
    let mut client = RECT::default();
    if unsafe { GetClientRect(hwnd, &mut client) }.is_err() {
        return;
    }
    state.image_generation = state.image_generation.wrapping_add(1);
    state.requested_path = Some(path.clone());
    state.renderer.set_loading(&path);
    spawn_image_decode(
        hwnd,
        path,
        (client.right - client.left).max(1) as u32,
        (client.bottom - client.top).max(1) as u32,
        state.image_sender.clone(),
        state.image_generation,
    );
}

fn open_thumbnail(hwnd: HWND, state: &mut WindowState, index: usize) {
    if let Some(path) = state.renderer.thumbnail_path(index) {
        state.renderer.select_thumbnail(index);
        request_image(hwnd, state, path);
    }
}

fn delete_current_image(hwnd: HWND, state: &mut WindowState) {
    if !state.renderer.has_image() {
        return;
    }
    let Some(path) = state.requested_path.clone() else {
        return;
    };
    if let Err(error) = move_file_to_recycle_bin(hwnd, &path) {
        state
            .renderer
            .set_status_message(format!("无法移入回收站：{error}"));
        let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
        return;
    }

    state.image_generation = state.image_generation.wrapping_add(1);
    state.directory_generation = state.directory_generation.wrapping_add(1);
    state.thumbnail_loader.replace_pending(Vec::new());
    let replacement = state.renderer.remove_thumbnail_and_select_neighbor(&path);
    if let Some(replacement) = replacement {
        request_image(hwnd, state, replacement);
    } else {
        state.requested_path = None;
        state.renderer.clear_image();
    }
    queue_thumbnail_requests(state);
    let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
}

fn save_thumbnail_preferences(state: &WindowState) {
    let (visible, dock) = state.renderer.thumbnail_preferences();
    let _ = registry::save_thumbnail_preferences(ThumbnailPreferences { visible, dock });
}

fn queue_thumbnail_requests(state: &mut WindowState) {
    let requests = state.renderer.thumbnail_requests();
    let tasks: Vec<_> = requests
        .iter()
        .map(|request| ThumbnailTask {
            generation: state.directory_generation,
            index: request.index,
            path: request.path.clone(),
            target_size_px: request.target_size_px,
        })
        .collect();
    state.thumbnail_loader.replace_pending(tasks);
    for request in requests {
        state
            .renderer
            .mark_thumbnail_queued(request.index, &request.path);
    }
}

fn update_navigation_timer(hwnd: HWND, state: &WindowState) {
    if state.renderer.navigation_animation_active() {
        let _ = unsafe {
            SetTimer(
                Some(hwnd),
                NAVIGATION_TIMER_ID,
                NAVIGATION_TIMER_INTERVAL_MS,
                None,
            )
        };
    } else {
        let _ = unsafe { KillTimer(Some(hwnd), NAVIGATION_TIMER_ID) };
    }
}

fn enforce_minimum_window_size(bounds: &mut MINMAXINFO) {
    bounds.ptMinTrackSize.x = MINIMUM_WIDTH;
    bounds.ptMinTrackSize.y = MINIMUM_HEIGHT;
}

fn enforce_window_bounds(hwnd: HWND, bounds: &mut MINMAXINFO) {
    enforce_minimum_window_size(bounds);
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut monitor_info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(monitor, &mut monitor_info) }.as_bool() {
        apply_maximized_work_area(bounds, monitor_info.rcMonitor, monitor_info.rcWork);
    }
}

fn apply_maximized_work_area(bounds: &mut MINMAXINFO, monitor: RECT, work_area: RECT) {
    bounds.ptMaxPosition.x = work_area.left - monitor.left;
    bounds.ptMaxPosition.y = work_area.top - monitor.top;
    bounds.ptMaxSize.x = work_area.right - work_area.left;
    bounds.ptMaxSize.y = work_area.bottom - work_area.top;
}

fn adjust_maximized_client_rect(hwnd: HWND, rect: &mut RECT) -> bool {
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32;
    if !unsafe { IsZoomed(hwnd) }.as_bool() || style & WS_THICKFRAME.0 == 0 {
        return false;
    }
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(1);
    let frame_x = unsafe { GetSystemMetricsForDpi(SM_CXFRAME, dpi) }
        + unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
    let frame_y = unsafe { GetSystemMetricsForDpi(SM_CYFRAME, dpi) }
        + unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
    inset_client_rect(rect, frame_x, frame_y);
    true
}

fn inset_client_rect(rect: &mut RECT, frame_x: i32, frame_y: i32) {
    rect.left += frame_x;
    rect.top += frame_y;
    rect.right -= frame_x;
    rect.bottom -= frame_y;
}

fn saved_window_state(hwnd: HWND, state: &WindowState) -> Option<SavedWindowState> {
    let placement = if state.fullscreen {
        state.windowed_placement?
    } else {
        let mut placement = WINDOWPLACEMENT {
            length: size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        unsafe { GetWindowPlacement(hwnd, &mut placement) }.ok()?;
        placement
    };
    Some(saved_window_state_from_placement(
        placement,
        if state.fullscreen {
            placement.showCmd == SW_SHOWMAXIMIZED.0 as u32
        } else {
            unsafe { IsZoomed(hwnd) }.as_bool()
        },
    ))
}

fn saved_window_state_from_placement(
    placement: WINDOWPLACEMENT,
    maximized: bool,
) -> SavedWindowState {
    SavedWindowState {
        width: (placement.rcNormalPosition.right - placement.rcNormalPosition.left)
            .max(MINIMUM_WIDTH) as u32,
        height: (placement.rcNormalPosition.bottom - placement.rcNormalPosition.top)
            .max(MINIMUM_HEIGHT) as u32,
        maximized,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimum_window_tracking_size_is_890_square() {
        let mut bounds = MINMAXINFO::default();
        enforce_minimum_window_size(&mut bounds);
        assert_eq!(bounds.ptMinTrackSize.x, 890);
        assert_eq!(bounds.ptMinTrackSize.y, 890);
    }

    #[test]
    fn maximized_window_uses_the_monitor_work_area() {
        let mut bounds = MINMAXINFO::default();
        apply_maximized_work_area(
            &mut bounds,
            RECT {
                left: -1920,
                top: 0,
                right: 0,
                bottom: 1080,
            },
            RECT {
                left: -1920,
                top: 0,
                right: 0,
                bottom: 1040,
            },
        );
        assert_eq!(bounds.ptMaxPosition.x, 0);
        assert_eq!(bounds.ptMaxPosition.y, 0);
        assert_eq!(bounds.ptMaxSize.x, 1920);
        assert_eq!(bounds.ptMaxSize.y, 1040);
    }

    #[test]
    fn maximized_client_excludes_the_invisible_resize_frame() {
        let mut rect = RECT {
            left: -12,
            top: -12,
            right: 3852,
            bottom: 2076,
        };
        inset_client_rect(&mut rect, 12, 12);
        assert_eq!(
            rect,
            RECT {
                left: 0,
                top: 0,
                right: 3840,
                bottom: 2064,
            }
        );
    }

    #[test]
    fn saved_window_state_uses_normal_size_and_maximized_flag() {
        let placement = WINDOWPLACEMENT {
            rcNormalPosition: RECT {
                left: 100,
                top: 200,
                right: 1700,
                bottom: 1200,
            },
            ..Default::default()
        };
        assert_eq!(
            saved_window_state_from_placement(placement, true),
            SavedWindowState {
                width: 1600,
                height: 1000,
                maximized: true,
            }
        );
    }
}
