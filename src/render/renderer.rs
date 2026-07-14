use crate::image::DecodedImage;
use crate::render::icons::IconSet;
use purepic::ui::chrome::{
    CAPTION_BUTTON_WIDTH_DIP, CaptionButton, title_action_button_rect, title_action_separator_x,
};
use purepic::ui::controls::{StatusControl, StatusControlsLayout};
use purepic::ui::icon::Icon;
use purepic::ui::layout::{LayoutInput, RectF, WindowLayout, compute_layout};
use purepic::ui::zoom::{
    MAX_ZOOM, MIN_ZOOM, SizeF, fit_zoom, slider_to_zoom, step_zoom, zoom_to_slider,
};
use windows::Win32::Foundation::{E_FAIL, HMODULE, HWND};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_IGNORE, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F,
    D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED, D2D1_FILL_MODE_WINDING, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_OPTIONS_TARGET,
    D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_ELLIPSE, D2D1_FACTORY_OPTIONS, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_INTERPOLATION_MODE_LINEAR, D2D1_ROUNDED_RECT, D2D1CreateFactory, ID2D1Bitmap1, ID2D1Brush,
    ID2D1Device, ID2D1DeviceContext, ID2D1Factory1, ID2D1Image, ID2D1SolidColorBrush,
    ID2D1StrokeStyle,
};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_WORD_WRAPPING_NO_WRAP,
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_MWA_NO_ALT_ENTER, DXGI_PRESENT, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1,
    DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT,
    IDXGIAdapter, IDXGIDevice, IDXGIFactory2, IDXGIOutput, IDXGISurface, IDXGISwapChain1,
};
use windows::core::{Error, Interface, Result, w};
use windows_numerics::Vector2;

const BACKGROUND: D2D1_COLOR_F = color(0x20, 0x20, 0x20);
const STATUS_BACKGROUND: D2D1_COLOR_F = color(0x27, 0x27, 0x27);
const STATUS_CONTROL_BACKGROUND: D2D1_COLOR_F = color(0x34, 0x34, 0x34);
const MENU_BACKGROUND: D2D1_COLOR_F = color(0x2E, 0x2E, 0x2E);
const MENU_HOVER_BACKGROUND: D2D1_COLOR_F = color(0x3A, 0x3A, 0x3A);
const MENU_SELECTED_BACKGROUND: D2D1_COLOR_F = color(0x17, 0x6F, 0x71);
const PRIMARY_TEXT: D2D1_COLOR_F = color(0xF4, 0xF6, 0xF8);
const SECONDARY_TEXT: D2D1_COLOR_F = color(0xB4, 0xBC, 0xC2);
const MUTED_TEXT: D2D1_COLOR_F = color(0x73, 0x7E, 0x85);
const CAPTION_HOVER: D2D1_COLOR_F = color(0x31, 0x3A, 0x3F);
const CAPTION_CLOSE_HOVER: D2D1_COLOR_F = color(0xC4, 0x2B, 0x1C);
const ACCENT: D2D1_COLOR_F = color(0x28, 0xD7, 0xE2);
const APP_TITLE: &str = "PurePic 图片查看器";
const TITLE_TEXT_LEFT_DIP: f32 = 176.0;

const fn color(r: u8, g: u8, b: u8) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

pub struct Renderer {
    _d3d_device: ID3D11Device,
    d2d_factory: ID2D1Factory1,
    _d2d_device: ID2D1Device,
    context: ID2D1DeviceContext,
    swap_chain: IDXGISwapChain1,
    target: Option<ID2D1Bitmap1>,
    title_brush: ID2D1SolidColorBrush,
    status_brush: ID2D1SolidColorBrush,
    status_control_brush: ID2D1SolidColorBrush,
    menu_brush: ID2D1SolidColorBrush,
    menu_hover_brush: ID2D1SolidColorBrush,
    menu_selected_brush: ID2D1SolidColorBrush,
    primary_text_brush: ID2D1SolidColorBrush,
    secondary_text_brush: ID2D1SolidColorBrush,
    muted_text_brush: ID2D1SolidColorBrush,
    caption_hover_brush: ID2D1SolidColorBrush,
    caption_close_hover_brush: ID2D1SolidColorBrush,
    accent_brush: ID2D1SolidColorBrush,
    title_format: IDWriteTextFormat,
    brand_format: IDWriteTextFormat,
    status_format: IDWriteTextFormat,
    tooltip_format: IDWriteTextFormat,
    message_format: IDWriteTextFormat,
    dpi: u32,
    width_px: u32,
    height_px: u32,
    caption_hot: CaptionButton,
    title_action_hot: bool,
    context_menu_registered: bool,
    maximized: bool,
    title: String,
    status: String,
    message: String,
    image: Option<RenderedImage>,
    icons: IconSet,
    zoom: f32,
    fit_mode: bool,
    zoom_menu_open: bool,
    fullscreen: bool,
    status_hot: Option<StatusControl>,
    zoom_menu_hot: Option<usize>,
    pan_x: f32,
    pan_y: f32,
    pan_last_position: Option<(f32, f32)>,
}

struct RenderedImage {
    bitmap: ID2D1Bitmap1,
    width: u32,
    height: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PointerAction {
    #[default]
    None,
    BeginSlider,
    BeginPan,
    ToggleFullscreen,
    ToggleContextMenu,
}

impl Renderer {
    pub fn new(hwnd: HWND, dpi: u32, width_px: u32, height_px: u32) -> Result<Self> {
        let d3d_device = create_d3d_device()?;
        let dxgi_device: IDXGIDevice = d3d_device.cast()?;

        let options = D2D1_FACTORY_OPTIONS::default();
        let d2d_factory: ID2D1Factory1 =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, Some(&options))? };
        let d2d_device = unsafe { d2d_factory.CreateDevice(&dxgi_device)? };
        let context = unsafe { d2d_device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)? };
        unsafe { context.SetDpi(dpi as f32, dpi as f32) };

        let adapter = unsafe { dxgi_device.GetAdapter()? };
        let dxgi_factory: IDXGIFactory2 = unsafe { adapter.GetParent()? };

        let swap_chain_description = DXGI_SWAP_CHAIN_DESC1 {
            Width: width_px,
            Height: height_px,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            Stereo: false.into(),
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            AlphaMode: DXGI_ALPHA_MODE_IGNORE,
            Flags: 0,
        };

        let swap_chain = unsafe {
            dxgi_factory.CreateSwapChainForHwnd(
                &d3d_device,
                hwnd,
                &swap_chain_description,
                None,
                None::<&IDXGIOutput>,
            )?
        };
        unsafe { dxgi_factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER)? };

        let write_factory: IDWriteFactory =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        let title_format = create_text_format(&write_factory, 15.0, DWRITE_TEXT_ALIGNMENT_CENTER)?;
        let brand_format = create_text_format(&write_factory, 14.0, DWRITE_TEXT_ALIGNMENT_LEADING)?;
        let status_format =
            create_text_format(&write_factory, 13.0, DWRITE_TEXT_ALIGNMENT_LEADING)?;
        let tooltip_format =
            create_text_format(&write_factory, 13.0, DWRITE_TEXT_ALIGNMENT_CENTER)?;
        let message_format =
            create_text_format(&write_factory, 15.0, DWRITE_TEXT_ALIGNMENT_CENTER)?;

        let title_brush = unsafe { context.CreateSolidColorBrush(&STATUS_BACKGROUND, None)? };
        let status_brush = unsafe { context.CreateSolidColorBrush(&STATUS_BACKGROUND, None)? };
        let status_control_brush =
            unsafe { context.CreateSolidColorBrush(&STATUS_CONTROL_BACKGROUND, None)? };
        let menu_brush = unsafe { context.CreateSolidColorBrush(&MENU_BACKGROUND, None)? };
        let menu_hover_brush =
            unsafe { context.CreateSolidColorBrush(&MENU_HOVER_BACKGROUND, None)? };
        let menu_selected_brush =
            unsafe { context.CreateSolidColorBrush(&MENU_SELECTED_BACKGROUND, None)? };
        let primary_text_brush = unsafe { context.CreateSolidColorBrush(&PRIMARY_TEXT, None)? };
        let secondary_text_brush = unsafe { context.CreateSolidColorBrush(&SECONDARY_TEXT, None)? };
        let muted_text_brush = unsafe { context.CreateSolidColorBrush(&MUTED_TEXT, None)? };
        let caption_hover_brush = unsafe { context.CreateSolidColorBrush(&CAPTION_HOVER, None)? };
        let caption_close_hover_brush =
            unsafe { context.CreateSolidColorBrush(&CAPTION_CLOSE_HOVER, None)? };
        let accent_brush = unsafe { context.CreateSolidColorBrush(&ACCENT, None)? };

        let mut renderer = Self {
            _d3d_device: d3d_device,
            d2d_factory,
            _d2d_device: d2d_device,
            context,
            swap_chain,
            target: None,
            title_brush,
            status_brush,
            status_control_brush,
            menu_brush,
            menu_hover_brush,
            menu_selected_brush,
            primary_text_brush,
            secondary_text_brush,
            muted_text_brush,
            caption_hover_brush,
            caption_close_hover_brush,
            accent_brush,
            title_format,
            brand_format,
            status_format,
            tooltip_format,
            message_format,
            dpi: dpi.max(1),
            width_px,
            height_px,
            caption_hot: CaptionButton::None,
            title_action_hot: false,
            context_menu_registered: false,
            maximized: false,
            title: "PurePic".to_owned(),
            status: "— × —     0 B".to_owned(),
            message: "Open an image to begin".to_owned(),
            image: None,
            icons: IconSet::load(),
            zoom: 1.0,
            fit_mode: true,
            zoom_menu_open: false,
            fullscreen: false,
            status_hot: None,
            zoom_menu_hot: None,
            pan_x: 0.0,
            pan_y: 0.0,
            pan_last_position: None,
        };
        renderer.create_target()?;
        Ok(renderer)
    }

    pub fn render(&self) -> Result<()> {
        if self.width_px == 0 || self.height_px == 0 || self.target.is_none() {
            return Ok(());
        }

        let layout = self.current_layout();
        let canvas_center = RectF::new(
            layout.canvas.x,
            layout.canvas.y + layout.canvas.height * 0.5 - 24.0,
            layout.canvas.width,
            48.0,
        );
        let status_left = RectF::new(
            layout.status_bar.x + 18.0,
            layout.status_bar.y,
            (layout.status_bar.width - 440.0).max(0.0),
            layout.status_bar.height,
        );
        let controls = StatusControlsLayout::compute(layout.status_bar);

        unsafe {
            self.context.BeginDraw();
            self.context.Clear(Some(&BACKGROUND));

            if let Some(image) = &self.image {
                if let Some(destination) = self.image_destination(layout.canvas) {
                    self.context.DrawBitmap(
                        &image.bitmap,
                        Some(&to_d2d_rect(destination)),
                        1.0,
                        D2D1_INTERPOLATION_MODE_LINEAR,
                        None,
                        None,
                    );
                }
            }

            if !self.message.is_empty() {
                draw_text(
                    &self.context,
                    &self.message,
                    &self.message_format,
                    canvas_center,
                    &self.muted_text_brush,
                );
            }
            if !self.fullscreen {
                // The chrome is an overlay: large, zoomed images may extend beyond the
                // canvas, but must never obscure the title or status bars.
                self.context
                    .FillRectangle(&to_d2d_rect(layout.title_bar), &self.title_brush);
                self.context
                    .FillRectangle(&to_d2d_rect(layout.status_bar), &self.status_brush);
                let title_text = title_text_rect(layout.title_bar);
                if title_text.width > 0.0 {
                    draw_text(
                        &self.context,
                        &self.title,
                        &self.title_format,
                        title_text,
                        &self.primary_text_brush,
                    );
                }
                self.draw_title_brand(layout.title_bar);
                self.draw_title_action(layout.title_bar);
                self.draw_caption_buttons(layout.title_bar);
                draw_text(
                    &self.context,
                    &self.status,
                    &self.status_format,
                    status_left,
                    &self.secondary_text_brush,
                );
                self.draw_status_controls(controls, layout.canvas);
                if self.zoom_menu_open {
                    self.draw_zoom_menu(controls.zoom_menu, layout.canvas);
                }
                self.draw_status_tooltip(controls, layout.canvas);
                self.draw_title_action_tooltip(layout.title_bar, layout.canvas);
            }

            self.context.EndDraw(None, None)?;
            self.swap_chain.Present(1, DXGI_PRESENT(0)).ok()?;
        }

        Ok(())
    }

    pub fn resize(&mut self, width_px: u32, height_px: u32) -> Result<()> {
        self.width_px = width_px;
        self.height_px = height_px;

        if width_px == 0 || height_px == 0 {
            return Ok(());
        }

        unsafe {
            self.context.SetTarget(None::<&ID2D1Image>);
        }
        self.target = None;

        unsafe {
            self.swap_chain.ResizeBuffers(
                0,
                width_px,
                height_px,
                DXGI_FORMAT_UNKNOWN,
                DXGI_SWAP_CHAIN_FLAG(0),
            )?;
        }
        self.create_target()
    }

    pub fn set_dpi(&mut self, dpi: u32) {
        self.dpi = dpi.max(1);
        unsafe {
            self.context.SetDpi(self.dpi as f32, self.dpi as f32);
        }
    }

    pub fn set_caption_hot(&mut self, button: CaptionButton) {
        self.caption_hot = button;
    }

    pub fn set_context_menu_registered(&mut self, registered: bool) {
        self.context_menu_registered = registered;
    }

    pub fn set_maximized(&mut self, maximized: bool) {
        self.maximized = maximized;
    }

    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = fullscreen;
        self.zoom_menu_open = false;
        self.status_hot = None;
        self.zoom_menu_hot = None;
        self.title_action_hot = false;
    }

    pub fn close_zoom_menu(&mut self) -> bool {
        self.zoom_menu_hot = None;
        std::mem::take(&mut self.zoom_menu_open)
    }

    pub fn set_pointer_hot(&mut self, x_px: i32, y_px: i32) -> bool {
        let (x, y) = self.point_to_dip(x_px, y_px);
        let layout = self.current_layout();
        let status_hot = if self.fullscreen {
            None
        } else {
            StatusControlsLayout::compute(layout.status_bar)
                .hit_test(x, y)
                .filter(|control| control.tooltip().is_some())
        };
        let zoom_menu_hot = if self.zoom_menu_open && !self.fullscreen {
            let controls = StatusControlsLayout::compute(layout.status_bar);
            zoom_choice_index_at(self.zoom_menu_rect(controls.zoom_menu), x, y)
        } else {
            None
        };
        let title_action_hot =
            !self.fullscreen && title_action_button_rect(layout.title_bar).contains(x, y);
        if self.status_hot == status_hot
            && self.zoom_menu_hot == zoom_menu_hot
            && self.title_action_hot == title_action_hot
        {
            return false;
        }
        self.status_hot = status_hot;
        self.zoom_menu_hot = zoom_menu_hot;
        self.title_action_hot = title_action_hot;
        true
    }

    pub fn clear_pointer_hot(&mut self) -> bool {
        let changed =
            self.status_hot.is_some() || self.zoom_menu_hot.is_some() || self.title_action_hot;
        self.status_hot = None;
        self.zoom_menu_hot = None;
        self.title_action_hot = false;
        changed
    }

    pub fn pointer_down(&mut self, x_px: i32, y_px: i32) -> PointerAction {
        let (x, y) = self.point_to_dip(x_px, y_px);
        let layout = self.current_layout();
        if self.fullscreen {
            return self.begin_pan(x, y, layout.canvas);
        }
        if title_action_button_rect(layout.title_bar).contains(x, y) {
            return PointerAction::ToggleContextMenu;
        }
        let controls = StatusControlsLayout::compute(layout.status_bar);

        if self.zoom_menu_open {
            if let Some(choice) = zoom_choice_at(self.zoom_menu_rect(controls.zoom_menu), x, y) {
                self.apply_zoom_choice(choice);
                self.zoom_menu_open = false;
                self.zoom_menu_hot = None;
                return PointerAction::None;
            }
            self.zoom_menu_open = false;
            self.zoom_menu_hot = None;
            return PointerAction::None;
        }

        match controls.hit_test(x, y) {
            Some(StatusControl::ActualSize) => {
                self.fit_mode = false;
                self.zoom = 1.0;
            }
            Some(StatusControl::ZoomMenu) => {
                self.zoom_menu_open = true;
                self.zoom_menu_hot = None;
            }
            Some(StatusControl::ZoomOut) => {
                self.zoom = step_zoom(self.current_zoom(layout.canvas) as f64, -1) as f32;
                self.fit_mode = false;
            }
            Some(StatusControl::Slider) => {
                self.set_slider_from_x(controls.slider, x);
                return PointerAction::BeginSlider;
            }
            Some(StatusControl::ZoomIn) => {
                self.zoom = step_zoom(self.current_zoom(layout.canvas) as f64, 1) as f32;
                self.fit_mode = false;
            }
            Some(StatusControl::Fullscreen) => return PointerAction::ToggleFullscreen,
            None => return self.begin_pan(x, y, layout.canvas),
        }
        PointerAction::None
    }

    pub fn pointer_move_slider(&mut self, x_px: i32) {
        let (x, _) = self.point_to_dip(x_px, 0);
        let layout = self.current_layout();
        self.set_slider_from_x(StatusControlsLayout::compute(layout.status_bar).slider, x);
    }

    pub fn pointer_move_pan(&mut self, x_px: i32, y_px: i32) {
        let (x, y) = self.point_to_dip(x_px, y_px);
        let Some((last_x, last_y)) = self.pan_last_position else {
            return;
        };
        self.pan_x += x - last_x;
        self.pan_y += y - last_y;
        self.pan_last_position = Some((x, y));
        self.constrain_pan();
    }

    pub fn end_pan(&mut self) {
        self.pan_last_position = None;
    }

    pub fn shows_pan_cursor(&self, x_px: i32, y_px: i32) -> bool {
        let (x, y) = self.point_to_dip(x_px, y_px);
        let canvas = self.current_layout().canvas;
        if !canvas.contains(x, y) {
            return false;
        }
        let Some((width, height)) = self.image_size(canvas) else {
            return false;
        };
        if !image_exceeds_canvas(canvas, width, height) {
            return false;
        }
        self.image_destination(canvas)
            .is_some_and(|destination| destination.contains(x, y))
    }

    pub fn set_loading(&mut self, path: &std::path::Path) {
        self.title = display_file_name(path);
        self.message = "Loading image…".to_owned();
        self.status = "Reading metadata…".to_owned();
        self.image = None;
    }

    pub fn set_image(&mut self, image: DecodedImage) -> Result<()> {
        let properties = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
            ..Default::default()
        };
        let bitmap = unsafe {
            self.context.CreateBitmap(
                D2D_SIZE_U {
                    width: image.width,
                    height: image.height,
                },
                Some(image.pixels.as_ptr().cast()),
                image.stride,
                &properties,
            )?
        };

        self.title = image.file_name;
        let size_label = if image.file_size == 0 {
            "内置演示图".to_owned()
        } else {
            format_file_size(image.file_size)
        };
        self.status = format!(
            "{} × {}     {}",
            image.original_width, image.original_height, size_label
        );
        self.message.clear();
        self.image = Some(RenderedImage {
            bitmap,
            width: image.original_width,
            height: image.original_height,
        });
        self.fit_mode = true;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
        self.pan_last_position = None;
        Ok(())
    }

    pub fn set_image_error(&mut self, path: &std::path::Path, error: &str) {
        self.title = display_file_name(path);
        self.status = "Unable to open image".to_owned();
        self.message = error.to_owned();
        self.image = None;
    }

    fn create_target(&mut self) -> Result<()> {
        if self.width_px == 0 || self.height_px == 0 {
            return Ok(());
        }

        let surface: IDXGISurface = unsafe { self.swap_chain.GetBuffer(0)? };
        let properties = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: self.dpi as f32,
            dpiY: self.dpi as f32,
            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
            ..Default::default()
        };
        let target = unsafe {
            self.context
                .CreateBitmapFromDxgiSurface(&surface, Some(&properties))?
        };
        unsafe {
            self.context.SetTarget(&target);
        }
        self.target = Some(target);
        Ok(())
    }

    unsafe fn draw_caption_buttons(&self, title_bar: RectF) {
        let close = RectF::new(
            (title_bar.right() - CAPTION_BUTTON_WIDTH_DIP).max(0.0),
            title_bar.y,
            CAPTION_BUTTON_WIDTH_DIP.min(title_bar.width),
            title_bar.height,
        );
        let maximize = RectF::new(
            (close.x - CAPTION_BUTTON_WIDTH_DIP).max(0.0),
            title_bar.y,
            CAPTION_BUTTON_WIDTH_DIP.min(close.x),
            title_bar.height,
        );
        let minimize = RectF::new(
            (maximize.x - CAPTION_BUTTON_WIDTH_DIP).max(0.0),
            title_bar.y,
            CAPTION_BUTTON_WIDTH_DIP.min(maximize.x),
            title_bar.height,
        );

        if self.caption_hot == CaptionButton::Minimize {
            unsafe {
                self.context
                    .FillRectangle(&to_d2d_rect(minimize), &self.caption_hover_brush)
            };
        }
        if self.caption_hot == CaptionButton::Maximize {
            unsafe {
                self.context
                    .FillRectangle(&to_d2d_rect(maximize), &self.caption_hover_brush)
            };
        }
        if self.caption_hot == CaptionButton::Close {
            unsafe {
                self.context
                    .FillRectangle(&to_d2d_rect(close), &self.caption_close_hover_brush)
            };
        }

        unsafe {
            draw_icon(
                &self.context,
                &self.d2d_factory,
                &self.icons.window_minimize,
                centered_square(minimize, 16.0),
                &self.primary_text_brush,
            );
            draw_icon(
                &self.context,
                &self.d2d_factory,
                if self.maximized {
                    &self.icons.window_restore
                } else {
                    &self.icons.window_maximize
                },
                centered_square(maximize, 16.0),
                &self.primary_text_brush,
            );
            draw_icon(
                &self.context,
                &self.d2d_factory,
                &self.icons.window_close,
                centered_square(close, 16.0),
                &self.primary_text_brush,
            );
        }
    }

    unsafe fn draw_title_brand(&self, title_bar: RectF) {
        let icon = RectF::new(
            title_bar.x + 10.0,
            title_bar.y + (title_bar.height - 20.0) * 0.5,
            20.0,
            20.0,
        );
        let label = RectF::new(icon.right() + 8.0, title_bar.y, 180.0, title_bar.height);
        unsafe {
            draw_icon(
                &self.context,
                &self.d2d_factory,
                &self.icons.app,
                icon,
                &self.accent_brush,
            );
            draw_text(
                &self.context,
                APP_TITLE,
                &self.brand_format,
                label,
                &self.primary_text_brush,
            );
        }
    }

    unsafe fn draw_title_action(&self, title_bar: RectF) {
        let button = title_action_button_rect(title_bar);
        if self.title_action_hot {
            unsafe {
                self.context
                    .FillRectangle(&to_d2d_rect(button), &self.caption_hover_brush)
            };
        }
        let separator = RectF::new(
            title_action_separator_x(title_bar) - 0.5,
            title_bar.y + 10.0,
            1.0,
            (title_bar.height - 20.0).max(0.0),
        );
        unsafe {
            self.context
                .FillRectangle(&to_d2d_rect(separator), &self.muted_text_brush);
            draw_icon(
                &self.context,
                &self.d2d_factory,
                if self.context_menu_registered {
                    &self.icons.context_unregister
                } else {
                    &self.icons.context_register
                },
                centered_square(button, 18.0),
                &self.primary_text_brush,
            );
        }
    }

    unsafe fn draw_title_action_tooltip(&self, title_bar: RectF, canvas: RectF) {
        if !self.title_action_hot {
            return;
        }
        let button = title_action_button_rect(title_bar);
        let width = 132.0;
        let rect = RectF::new(
            (button.x + (button.width - width) * 0.5)
                .clamp(canvas.x + 8.0, canvas.right() - width - 8.0),
            title_bar.bottom() + 8.0,
            width,
            32.0,
        );
        let label = if self.context_menu_registered {
            "取消图片右键菜单"
        } else {
            "注册图片右键菜单"
        };
        unsafe {
            self.context
                .FillRoundedRectangle(&to_d2d_rounded_rect(rect, 6.0), &self.title_brush);
            draw_text(
                &self.context,
                label,
                &self.tooltip_format,
                rect,
                &self.primary_text_brush,
            );
        }
    }

    unsafe fn draw_status_controls(&self, controls: StatusControlsLayout, canvas: RectF) {
        let icon_inset = 8.0;
        unsafe {
            self.context.FillRoundedRectangle(
                &to_d2d_rounded_rect(controls.zoom_menu, 8.0),
                &self.status_control_brush,
            );
            if let Some(control) = self.status_hot {
                self.context.FillRoundedRectangle(
                    &to_d2d_rounded_rect(controls.rect(control), 6.0),
                    &self.caption_hover_brush,
                );
            }
            draw_icon(
                &self.context,
                &self.d2d_factory,
                &self.icons.actual_size,
                inset(controls.actual_size, icon_inset),
                &self.primary_text_brush,
            );
            let label_rect = RectF::new(
                controls.zoom_menu.x + 5.0,
                controls.zoom_menu.y,
                controls.zoom_menu.width - 24.0,
                controls.zoom_menu.height,
            );
            draw_text(
                &self.context,
                &format!("{:.0}%", self.current_zoom(canvas) * 100.0),
                &self.title_format,
                label_rect,
                &self.primary_text_brush,
            );
            let chevron_rect = RectF::new(
                controls.zoom_menu.right() - 20.0,
                controls.zoom_menu.y + (controls.zoom_menu.height - 12.0) * 0.5,
                12.0,
                12.0,
            );
            draw_icon(
                &self.context,
                &self.d2d_factory,
                &self.icons.chevron_down,
                chevron_rect,
                &self.primary_text_brush,
            );
            draw_icon(
                &self.context,
                &self.d2d_factory,
                &self.icons.zoom_out,
                inset(controls.zoom_out, icon_inset),
                &self.primary_text_brush,
            );
            draw_icon(
                &self.context,
                &self.d2d_factory,
                &self.icons.zoom_in,
                inset(controls.zoom_in, icon_inset),
                &self.primary_text_brush,
            );
            draw_icon(
                &self.context,
                &self.d2d_factory,
                &self.icons.fullscreen,
                inset(controls.fullscreen, icon_inset),
                &self.primary_text_brush,
            );

            let track = RectF::new(
                controls.slider.x + 10.0,
                controls.slider.y + 16.0,
                controls.slider.width - 20.0,
                4.0,
            );
            self.context
                .FillRoundedRectangle(&to_d2d_rounded_rect(track, 2.0), &self.muted_text_brush);
            let position = zoom_to_slider(self.current_zoom(canvas) as f64) as f32;
            let filled = RectF::new(track.x, track.y, track.width * position, track.height);
            let knob_x = track.x + track.width * position;
            if filled.width > 0.0 {
                self.context
                    .FillRoundedRectangle(&to_d2d_rounded_rect(filled, 2.0), &self.accent_brush);
            }
            let center = Vector2 {
                X: knob_x,
                Y: controls.slider.y + controls.slider.height * 0.5,
            };
            self.context.FillEllipse(
                &D2D1_ELLIPSE {
                    point: center,
                    radiusX: 10.0,
                    radiusY: 10.0,
                },
                &self.caption_hover_brush,
            );
            self.context.FillEllipse(
                &D2D1_ELLIPSE {
                    point: center,
                    radiusX: 6.0,
                    radiusY: 6.0,
                },
                &self.accent_brush,
            );
        }
    }

    unsafe fn draw_zoom_menu(&self, button: RectF, canvas: RectF) {
        let menu = self.zoom_menu_rect(button);
        unsafe {
            self.context
                .FillRoundedRectangle(&to_d2d_rounded_rect(menu, 8.0), &self.menu_brush)
        };
        let current = self.current_zoom(canvas);
        for (index, choice) in ZOOM_CHOICES.iter().copied().enumerate() {
            let row = RectF::new(menu.x, menu.y + index as f32 * 30.0, menu.width, 30.0);
            let hovered = self.zoom_menu_hot == Some(index);
            let selected = match choice {
                ZoomChoice::Fit => self.fit_mode,
                ZoomChoice::Percent(value) => !self.fit_mode && (current - value).abs() < 0.001,
            };
            let state_brush = if selected {
                Some(&self.menu_selected_brush)
            } else if hovered {
                Some(&self.menu_hover_brush)
            } else {
                None
            };
            if let Some(brush) = state_brush {
                let selected_row = RectF::new(row.x + 4.0, row.y + 2.0, row.width - 8.0, 26.0);
                unsafe {
                    self.context
                        .FillRoundedRectangle(&to_d2d_rounded_rect(selected_row, 5.0), brush)
                };
            }
            unsafe {
                draw_text(
                    &self.context,
                    zoom_choice_label(choice),
                    &self.title_format,
                    row,
                    &self.primary_text_brush,
                )
            };
        }
    }

    unsafe fn draw_status_tooltip(&self, controls: StatusControlsLayout, canvas: RectF) {
        if self.zoom_menu_open {
            return;
        }
        let Some(control) = self.status_hot else {
            return;
        };
        let Some((label, width)) = control.tooltip() else {
            return;
        };
        let button = controls.rect(control);
        let rect = RectF::new(
            (button.x + (button.width - width) * 0.5)
                .clamp(canvas.x + 8.0, canvas.right() - width - 8.0),
            button.y - 40.0,
            width,
            32.0,
        );
        unsafe {
            self.context
                .FillRoundedRectangle(&to_d2d_rounded_rect(rect, 6.0), &self.title_brush);
            draw_text(
                &self.context,
                label,
                &self.tooltip_format,
                rect,
                &self.primary_text_brush,
            );
        }
    }

    fn current_zoom(&self, canvas: RectF) -> f32 {
        if !self.fit_mode {
            return self.zoom.clamp(MIN_ZOOM as f32, MAX_ZOOM as f32);
        }
        let Some(image) = &self.image else { return 1.0 };
        let scale = self.dpi as f64 / 96.0;
        fit_zoom(
            SizeF::new(image.width as f64, image.height as f64),
            SizeF::new(canvas.width as f64 * scale, canvas.height as f64 * scale),
        ) as f32
    }

    fn current_layout(&self) -> WindowLayout {
        let mut layout = compute_layout(LayoutInput::new(self.width_px, self.height_px, self.dpi));
        if self.fullscreen {
            layout.canvas = layout.client;
        }
        layout
    }

    fn image_destination(&self, canvas: RectF) -> Option<RectF> {
        let (width, height) = self.image_size(canvas)?;
        let (pan_x, pan_y) = self.constrained_pan_for(canvas, width, height);
        Some(RectF::new(
            canvas.x + (canvas.width - width) * 0.5 + pan_x,
            canvas.y + (canvas.height - height) * 0.5 + pan_y,
            width,
            height,
        ))
    }

    fn image_size(&self, canvas: RectF) -> Option<(f32, f32)> {
        let image = self.image.as_ref()?;
        let zoom = self.current_zoom(canvas);
        let dip_per_pixel = 96.0 / self.dpi as f32;
        Some((
            image.width as f32 * zoom * dip_per_pixel,
            image.height as f32 * zoom * dip_per_pixel,
        ))
    }

    fn begin_pan(&mut self, x: f32, y: f32, canvas: RectF) -> PointerAction {
        if !canvas.contains(x, y) {
            return PointerAction::None;
        }
        let Some((width, height)) = self.image_size(canvas) else {
            return PointerAction::None;
        };
        if !image_exceeds_canvas(canvas, width, height) {
            return PointerAction::None;
        }
        let (pan_x, pan_y) = self.constrained_pan_for(canvas, width, height);
        let destination = RectF::new(
            canvas.x + (canvas.width - width) * 0.5 + pan_x,
            canvas.y + (canvas.height - height) * 0.5 + pan_y,
            width,
            height,
        );
        if !destination.contains(x, y) {
            return PointerAction::None;
        }
        self.pan_last_position = Some((x, y));
        PointerAction::BeginPan
    }

    fn constrain_pan(&mut self) {
        let canvas = self.current_layout().canvas;
        let Some((width, height)) = self.image_size(canvas) else {
            self.pan_x = 0.0;
            self.pan_y = 0.0;
            return;
        };
        (self.pan_x, self.pan_y) = self.constrained_pan_for(canvas, width, height);
    }

    fn constrained_pan_for(&self, canvas: RectF, width: f32, height: f32) -> (f32, f32) {
        (
            constrain_pan_axis(self.pan_x, width, canvas.width),
            constrain_pan_axis(self.pan_y, height, canvas.height),
        )
    }

    fn point_to_dip(&self, x: i32, y: i32) -> (f32, f32) {
        let scale = 96.0 / self.dpi as f32;
        (x as f32 * scale, y as f32 * scale)
    }

    fn set_slider_from_x(&mut self, slider: RectF, x: f32) {
        let track_left = slider.x + 10.0;
        let track_width = (slider.width - 20.0).max(1.0);
        let position = ((x - track_left) / track_width).clamp(0.0, 1.0);
        self.zoom = slider_to_zoom(position as f64) as f32;
        self.fit_mode = false;
        self.zoom_menu_open = false;
    }

    fn zoom_menu_rect(&self, button: RectF) -> RectF {
        let height = ZOOM_CHOICES.len() as f32 * 30.0;
        RectF::new(button.right() - 88.0, button.y - height - 6.0, 88.0, height)
    }

    fn apply_zoom_choice(&mut self, choice: ZoomChoice) {
        match choice {
            ZoomChoice::Fit => self.fit_mode = true,
            ZoomChoice::Percent(value) => {
                self.zoom = value;
                self.fit_mode = false;
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ZoomChoice {
    Fit,
    Percent(f32),
}

const ZOOM_CHOICES: [ZoomChoice; 10] = [
    ZoomChoice::Fit,
    ZoomChoice::Percent(0.10),
    ZoomChoice::Percent(0.25),
    ZoomChoice::Percent(0.50),
    ZoomChoice::Percent(0.75),
    ZoomChoice::Percent(1.00),
    ZoomChoice::Percent(1.50),
    ZoomChoice::Percent(2.00),
    ZoomChoice::Percent(4.00),
    ZoomChoice::Percent(8.00),
];

fn zoom_choice_label(choice: ZoomChoice) -> &'static str {
    match choice {
        ZoomChoice::Fit => "适应窗口",
        ZoomChoice::Percent(0.10) => "10%",
        ZoomChoice::Percent(0.25) => "25%",
        ZoomChoice::Percent(0.50) => "50%",
        ZoomChoice::Percent(0.75) => "75%",
        ZoomChoice::Percent(1.00) => "100%",
        ZoomChoice::Percent(1.50) => "150%",
        ZoomChoice::Percent(2.00) => "200%",
        ZoomChoice::Percent(4.00) => "400%",
        ZoomChoice::Percent(8.00) => "800%",
        ZoomChoice::Percent(_) => "自定义",
    }
}

fn zoom_choice_at(menu: RectF, x: f32, y: f32) -> Option<ZoomChoice> {
    zoom_choice_index_at(menu, x, y).and_then(|index| ZOOM_CHOICES.get(index).copied())
}

fn zoom_choice_index_at(menu: RectF, x: f32, y: f32) -> Option<usize> {
    if !menu.contains(x, y) {
        return None;
    }
    let index = ((y - menu.y) / 30.0).floor() as usize;
    (index < ZOOM_CHOICES.len()).then_some(index)
}

fn inset(rect: RectF, amount: f32) -> RectF {
    RectF::new(
        rect.x + amount,
        rect.y + amount,
        (rect.width - amount * 2.0).max(0.0),
        (rect.height - amount * 2.0).max(0.0),
    )
}

fn centered_square(rect: RectF, size: f32) -> RectF {
    let extent = size.min(rect.width).min(rect.height).max(0.0);
    RectF::new(
        rect.x + (rect.width - extent) * 0.5,
        rect.y + (rect.height - extent) * 0.5,
        extent,
        extent,
    )
}

fn title_text_rect(title_bar: RectF) -> RectF {
    let left = title_bar.x + TITLE_TEXT_LEFT_DIP;
    let right = title_action_button_rect(title_bar).x - 8.0;
    RectF::new(left, title_bar.y, (right - left).max(0.0), title_bar.height)
}

unsafe fn draw_icon(
    context: &ID2D1DeviceContext,
    factory: &ID2D1Factory1,
    icon: &Icon,
    target: RectF,
    brush: &ID2D1SolidColorBrush,
) {
    if icon.width <= 0.0 || icon.height <= 0.0 || icon.paths.is_empty() {
        return;
    }
    let scale = (target.width / icon.width).min(target.height / icon.height);
    let origin_x = target.x + (target.width - icon.width * scale) * 0.5;
    let origin_y = target.y + (target.height - icon.height * scale) * 0.5;
    for path in &icon.paths {
        if path.fill {
            unsafe {
                fill_icon_path(
                    context,
                    factory,
                    &path.segments,
                    origin_x,
                    origin_y,
                    scale,
                    brush,
                );
            }
        }
        if path.stroke && path.stroke_width > 0.0 {
            let stroke_width = path.stroke_width * scale;
            for segment in &path.segments {
                let start = transform_icon_point(segment.start, origin_x, origin_y, scale);
                let end = transform_icon_point(segment.end, origin_x, origin_y, scale);
                unsafe {
                    context.DrawLine(start, end, brush, stroke_width, None::<&ID2D1StrokeStyle>);
                    let radius = stroke_width * 0.5;
                    context.FillEllipse(
                        &D2D1_ELLIPSE {
                            point: start,
                            radiusX: radius,
                            radiusY: radius,
                        },
                        brush,
                    );
                    context.FillEllipse(
                        &D2D1_ELLIPSE {
                            point: end,
                            radiusX: radius,
                            radiusY: radius,
                        },
                        brush,
                    );
                }
            }
        }
    }
}

unsafe fn fill_icon_path(
    context: &ID2D1DeviceContext,
    factory: &ID2D1Factory1,
    segments: &[purepic::ui::icon::IconSegment],
    origin_x: f32,
    origin_y: f32,
    scale: f32,
    brush: &ID2D1SolidColorBrush,
) {
    let Ok(geometry) = (unsafe { factory.CreatePathGeometry() }) else {
        return;
    };
    let Ok(sink) = (unsafe { geometry.Open() }) else {
        return;
    };
    unsafe { sink.SetFillMode(D2D1_FILL_MODE_WINDING) };

    let mut figure_start = 0;
    while figure_start < segments.len() {
        let mut figure_end = figure_start + 1;
        while figure_end < segments.len()
            && segments[figure_end - 1].end == segments[figure_end].start
        {
            figure_end += 1;
        }
        let first = transform_icon_point(segments[figure_start].start, origin_x, origin_y, scale);
        let points: Vec<_> = segments[figure_start..figure_end]
            .iter()
            .map(|segment| transform_icon_point(segment.end, origin_x, origin_y, scale))
            .collect();
        unsafe {
            sink.BeginFigure(first, D2D1_FIGURE_BEGIN_FILLED);
            sink.AddLines(&points);
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        }
        figure_start = figure_end;
    }
    if unsafe { sink.Close() }.is_ok() {
        unsafe { context.FillGeometry(&geometry, brush, None::<&ID2D1Brush>) };
    }
}

fn transform_icon_point(
    point: purepic::ui::icon::IconPoint,
    origin_x: f32,
    origin_y: f32,
    scale: f32,
) -> Vector2 {
    Vector2 {
        X: origin_x + point.x * scale,
        Y: origin_y + point.y * scale,
    }
}

fn constrain_pan_axis(pan: f32, image_extent: f32, canvas_extent: f32) -> f32 {
    if image_extent <= canvas_extent {
        return 0.0;
    }
    let limit = (image_extent - canvas_extent) * 0.5;
    pan.clamp(-limit, limit)
}

fn image_exceeds_canvas(canvas: RectF, width: f32, height: f32) -> bool {
    width > canvas.width || height > canvas.height
}

fn create_d3d_device() -> Result<ID3D11Device> {
    let mut device = None;
    let hardware_result = unsafe {
        D3D11CreateDevice(
            None::<&IDXGIAdapter>,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )
    };

    if hardware_result.is_err() {
        unsafe {
            D3D11CreateDevice(
                None::<&IDXGIAdapter>,
                D3D_DRIVER_TYPE_WARP,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                None,
            )?;
        }
    }

    device.ok_or_else(|| Error::new(E_FAIL, "D3D11 did not return a device"))
}

fn create_text_format(
    factory: &IDWriteFactory,
    size: f32,
    alignment: windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_ALIGNMENT,
) -> Result<IDWriteTextFormat> {
    let format = unsafe {
        factory.CreateTextFormat(
            w!("Segoe UI"),
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            w!("zh-CN"),
        )?
    };
    unsafe {
        format.SetTextAlignment(alignment)?;
        format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
    }
    Ok(format)
}

unsafe fn draw_text(
    context: &ID2D1DeviceContext,
    text: &str,
    format: &IDWriteTextFormat,
    rect: RectF,
    brush: &ID2D1SolidColorBrush,
) {
    let text: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        context.DrawText(
            &text,
            format,
            &to_d2d_rect(rect),
            brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
}

fn to_d2d_rect(rect: RectF) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.x,
        top: rect.y,
        right: rect.right(),
        bottom: rect.bottom(),
    }
}

fn to_d2d_rounded_rect(rect: RectF, radius: f32) -> D2D1_ROUNDED_RECT {
    D2D1_ROUNDED_RECT {
        rect: to_d2d_rect(rect),
        radiusX: radius,
        radiusY: radius,
    }
}

fn display_file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn format_file_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / MIB)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / KIB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_images_stop_at_the_canvas_edge_without_padding() {
        assert_eq!(constrain_pan_axis(1_000.0, 1200.0, 800.0), 200.0);
        assert_eq!(constrain_pan_axis(-1_000.0, 1200.0, 800.0), -200.0);
    }

    #[test]
    fn images_that_fit_do_not_pan_on_that_axis() {
        assert_eq!(constrain_pan_axis(50.0, 600.0, 800.0), 0.0);
    }

    #[test]
    fn image_is_pannable_when_either_axis_exceeds_the_canvas() {
        let canvas = RectF::new(0.0, 0.0, 800.0, 600.0);
        assert!(image_exceeds_canvas(canvas, 801.0, 500.0));
        assert!(image_exceeds_canvas(canvas, 700.0, 601.0));
        assert!(!image_exceeds_canvas(canvas, 800.0, 600.0));
    }

    #[test]
    fn zoom_menu_hover_maps_each_row_and_excludes_edges() {
        let menu = RectF::new(100.0, 50.0, 128.0, ZOOM_CHOICES.len() as f32 * 30.0);
        assert_eq!(zoom_choice_index_at(menu, 110.0, 50.0), Some(0));
        assert_eq!(zoom_choice_index_at(menu, 110.0, 139.9), Some(2));
        assert_eq!(
            zoom_choice_index_at(menu, 110.0, menu.bottom() - 0.1),
            Some(ZOOM_CHOICES.len() - 1)
        );
        assert_eq!(zoom_choice_index_at(menu, 110.0, menu.bottom()), None);
        assert_eq!(zoom_choice_index_at(menu, menu.right(), 60.0), None);
    }

    #[test]
    fn title_text_stays_between_brand_and_caption_buttons() {
        let bar = RectF::new(0.0, 0.0, 890.0, 44.0);
        let text = title_text_rect(bar);
        assert_eq!(text.x, 176.0);
        assert_eq!(text.right(), 696.0);

        let narrow = title_text_rect(RectF::new(0.0, 0.0, 300.0, 44.0));
        assert_eq!(narrow.width, 0.0);
    }
}
