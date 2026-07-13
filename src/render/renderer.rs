use crate::image::DecodedImage;
use crate::render::icons::IconSet;
use purepic::ui::chrome::CaptionButton;
use purepic::ui::controls::{StatusControl, StatusControlsLayout};
use purepic::ui::icon::Icon;
use purepic::ui::layout::{LayoutInput, RectF, WindowLayout, compute_layout};
use purepic::ui::zoom::{
    MAX_ZOOM, MIN_ZOOM, SizeF, fit_zoom, slider_to_zoom, step_zoom, zoom_to_slider,
};
use windows::Win32::Foundation::{E_FAIL, HMODULE, HWND};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_IGNORE, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F,
    D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_OPTIONS_TARGET,
    D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_FACTORY_OPTIONS, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_INTERPOLATION_MODE_LINEAR,
    D2D1CreateFactory, ID2D1Bitmap1, ID2D1Device, ID2D1DeviceContext, ID2D1Factory1, ID2D1Image,
    ID2D1SolidColorBrush, ID2D1StrokeStyle,
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

const BACKGROUND: D2D1_COLOR_F = color(0x0F, 0x14, 0x17);
const TITLE_BACKGROUND: D2D1_COLOR_F = color(0x18, 0x20, 0x24);
const STATUS_BACKGROUND: D2D1_COLOR_F = color(0x1B, 0x23, 0x27);
const PRIMARY_TEXT: D2D1_COLOR_F = color(0xF4, 0xF6, 0xF8);
const SECONDARY_TEXT: D2D1_COLOR_F = color(0xB4, 0xBC, 0xC2);
const MUTED_TEXT: D2D1_COLOR_F = color(0x73, 0x7E, 0x85);
const CAPTION_HOVER: D2D1_COLOR_F = color(0x31, 0x3A, 0x3F);
const CAPTION_CLOSE_HOVER: D2D1_COLOR_F = color(0xC4, 0x2B, 0x1C);
const ACCENT: D2D1_COLOR_F = color(0x28, 0xD7, 0xE2);

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
    _d2d_factory: ID2D1Factory1,
    _d2d_device: ID2D1Device,
    context: ID2D1DeviceContext,
    swap_chain: IDXGISwapChain1,
    target: Option<ID2D1Bitmap1>,
    title_brush: ID2D1SolidColorBrush,
    status_brush: ID2D1SolidColorBrush,
    primary_text_brush: ID2D1SolidColorBrush,
    secondary_text_brush: ID2D1SolidColorBrush,
    muted_text_brush: ID2D1SolidColorBrush,
    caption_hover_brush: ID2D1SolidColorBrush,
    caption_close_hover_brush: ID2D1SolidColorBrush,
    accent_brush: ID2D1SolidColorBrush,
    title_format: IDWriteTextFormat,
    status_format: IDWriteTextFormat,
    message_format: IDWriteTextFormat,
    dpi: u32,
    width_px: u32,
    height_px: u32,
    caption_hot: CaptionButton,
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
        let status_format =
            create_text_format(&write_factory, 13.0, DWRITE_TEXT_ALIGNMENT_LEADING)?;
        let message_format =
            create_text_format(&write_factory, 15.0, DWRITE_TEXT_ALIGNMENT_CENTER)?;

        let title_brush = unsafe { context.CreateSolidColorBrush(&TITLE_BACKGROUND, None)? };
        let status_brush = unsafe { context.CreateSolidColorBrush(&STATUS_BACKGROUND, None)? };
        let primary_text_brush = unsafe { context.CreateSolidColorBrush(&PRIMARY_TEXT, None)? };
        let secondary_text_brush = unsafe { context.CreateSolidColorBrush(&SECONDARY_TEXT, None)? };
        let muted_text_brush = unsafe { context.CreateSolidColorBrush(&MUTED_TEXT, None)? };
        let caption_hover_brush = unsafe { context.CreateSolidColorBrush(&CAPTION_HOVER, None)? };
        let caption_close_hover_brush =
            unsafe { context.CreateSolidColorBrush(&CAPTION_CLOSE_HOVER, None)? };
        let accent_brush = unsafe { context.CreateSolidColorBrush(&ACCENT, None)? };

        let mut renderer = Self {
            _d3d_device: d3d_device,
            _d2d_factory: d2d_factory,
            _d2d_device: d2d_device,
            context,
            swap_chain,
            target: None,
            title_brush,
            status_brush,
            primary_text_brush,
            secondary_text_brush,
            muted_text_brush,
            caption_hover_brush,
            caption_close_hover_brush,
            accent_brush,
            title_format,
            status_format,
            message_format,
            dpi: dpi.max(1),
            width_px,
            height_px,
            caption_hot: CaptionButton::None,
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
                draw_text(
                    &self.context,
                    &self.title,
                    &self.title_format,
                    layout.title_bar,
                    &self.primary_text_brush,
                );
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

    pub fn set_maximized(&mut self, maximized: bool) {
        self.maximized = maximized;
    }

    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = fullscreen;
        self.zoom_menu_open = false;
    }

    pub fn close_zoom_menu(&mut self) -> bool {
        std::mem::take(&mut self.zoom_menu_open)
    }

    pub fn pointer_down(&mut self, x_px: i32, y_px: i32) -> PointerAction {
        let (x, y) = self.point_to_dip(x_px, y_px);
        let layout = self.current_layout();
        if self.fullscreen {
            return self.begin_pan(x, y, layout.canvas);
        }
        let controls = StatusControlsLayout::compute(layout.status_bar);

        if self.zoom_menu_open {
            if let Some(choice) = zoom_choice_at(self.zoom_menu_rect(controls.zoom_menu), x, y) {
                self.apply_zoom_choice(choice);
                self.zoom_menu_open = false;
                return PointerAction::None;
            }
            self.zoom_menu_open = false;
        }

        match controls.hit_test(x, y) {
            Some(StatusControl::ActualSize) => {
                self.fit_mode = false;
                self.zoom = 1.0;
            }
            Some(StatusControl::ZoomMenu) => self.zoom_menu_open = true,
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
        const BUTTON_WIDTH: f32 = 46.0;
        let close = RectF::new(
            (title_bar.right() - BUTTON_WIDTH).max(0.0),
            title_bar.y,
            BUTTON_WIDTH.min(title_bar.width),
            title_bar.height,
        );
        let maximize = RectF::new(
            (close.x - BUTTON_WIDTH).max(0.0),
            title_bar.y,
            BUTTON_WIDTH.min(close.x),
            title_bar.height,
        );
        let minimize = RectF::new(
            (maximize.x - BUTTON_WIDTH).max(0.0),
            title_bar.y,
            BUTTON_WIDTH.min(maximize.x),
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
                &self.icons.window_minimize,
                inset(minimize, 10.0),
                &self.primary_text_brush,
                1.6,
            );
            draw_icon(
                &self.context,
                if self.maximized {
                    &self.icons.window_restore
                } else {
                    &self.icons.window_maximize
                },
                inset(maximize, 10.0),
                &self.primary_text_brush,
                1.6,
            );
            draw_icon(
                &self.context,
                &self.icons.window_close,
                inset(close, 10.0),
                &self.primary_text_brush,
                1.6,
            );
        }
    }

    unsafe fn draw_status_controls(&self, controls: StatusControlsLayout, canvas: RectF) {
        let icon_inset = 8.0;
        unsafe {
            draw_icon(
                &self.context,
                &self.icons.actual_size,
                inset(controls.actual_size, icon_inset),
                &self.primary_text_brush,
                1.7,
            );
            self.context
                .FillRectangle(&to_d2d_rect(controls.zoom_menu), &self.caption_hover_brush);
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
                controls.zoom_menu.y + 10.0,
                12.0,
                16.0,
            );
            draw_icon(
                &self.context,
                &self.icons.chevron_down,
                chevron_rect,
                &self.primary_text_brush,
                1.6,
            );
            draw_icon(
                &self.context,
                &self.icons.zoom_out,
                inset(controls.zoom_out, icon_inset),
                &self.primary_text_brush,
                1.7,
            );
            draw_icon(
                &self.context,
                &self.icons.zoom_in,
                inset(controls.zoom_in, icon_inset),
                &self.primary_text_brush,
                1.7,
            );
            draw_icon(
                &self.context,
                &self.icons.fullscreen,
                inset(controls.fullscreen, icon_inset),
                &self.primary_text_brush,
                1.7,
            );

            let track = RectF::new(
                controls.slider.x + 10.0,
                controls.slider.y + 17.0,
                controls.slider.width - 20.0,
                2.0,
            );
            self.context
                .FillRectangle(&to_d2d_rect(track), &self.muted_text_brush);
            let position = zoom_to_slider(self.current_zoom(canvas) as f64) as f32;
            let filled = RectF::new(track.x, track.y, track.width * position, track.height);
            self.context
                .FillRectangle(&to_d2d_rect(filled), &self.accent_brush);
            let knob_x = track.x + track.width * position;
            let knob = RectF::new(knob_x - 4.0, controls.slider.y + 9.0, 8.0, 18.0);
            self.context
                .FillRectangle(&to_d2d_rect(knob), &self.primary_text_brush);
        }
    }

    unsafe fn draw_zoom_menu(&self, button: RectF, canvas: RectF) {
        let menu = self.zoom_menu_rect(button);
        unsafe {
            self.context
                .FillRectangle(&to_d2d_rect(menu), &self.title_brush)
        };
        let current = self.current_zoom(canvas);
        for (index, choice) in ZOOM_CHOICES.iter().copied().enumerate() {
            let row = RectF::new(menu.x, menu.y + index as f32 * 30.0, menu.width, 30.0);
            let selected = match choice {
                ZoomChoice::Fit => self.fit_mode,
                ZoomChoice::Percent(value) => !self.fit_mode && (current - value).abs() < 0.001,
            };
            if selected {
                unsafe {
                    self.context
                        .FillRectangle(&to_d2d_rect(row), &self.caption_hover_brush)
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
        let Some((width, height)) = self.image_size(canvas) else {
            return PointerAction::None;
        };
        if width <= canvas.width && height <= canvas.height {
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
        let max_x = ((width - canvas.width) * 0.5).max(0.0);
        let max_y = ((height - canvas.height) * 0.5).max(0.0);
        (
            self.pan_x.clamp(-max_x, max_x),
            self.pan_y.clamp(-max_y, max_y),
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
        RectF::new(
            button.right() - 128.0,
            button.y - height - 6.0,
            128.0,
            height,
        )
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
    if !menu.contains(x, y) {
        return None;
    }
    let index = ((y - menu.y) / 30.0).floor() as usize;
    ZOOM_CHOICES.get(index).copied()
}

fn inset(rect: RectF, amount: f32) -> RectF {
    RectF::new(
        rect.x + amount,
        rect.y + amount,
        (rect.width - amount * 2.0).max(0.0),
        (rect.height - amount * 2.0).max(0.0),
    )
}

unsafe fn draw_icon(
    context: &ID2D1DeviceContext,
    icon: &Icon,
    target: RectF,
    brush: &ID2D1SolidColorBrush,
    stroke_width: f32,
) {
    if icon.width <= 0.0 || icon.height <= 0.0 || icon.segments.is_empty() {
        return;
    }
    let scale = (target.width / icon.width).min(target.height / icon.height);
    let origin_x = target.x + (target.width - icon.width * scale) * 0.5;
    let origin_y = target.y + (target.height - icon.height * scale) * 0.5;
    for segment in &icon.segments {
        unsafe {
            context.DrawLine(
                Vector2 {
                    X: origin_x + segment.start.x * scale,
                    Y: origin_y + segment.start.y * scale,
                },
                Vector2 {
                    X: origin_x + segment.end.x * scale,
                    Y: origin_y + segment.end.y * scale,
                },
                brush,
                stroke_width,
                None::<&ID2D1StrokeStyle>,
            );
        }
    }
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
