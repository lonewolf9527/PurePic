use crate::image::DecodedImage;
use crate::render::icons::IconSet;
use purepic::ui::chrome::{
    CAPTION_BUTTON_WIDTH_DIP, CaptionButton, default_app_button_rect, title_action_button_rect,
    title_action_separator_x,
};
use purepic::ui::controls::{
    StatusControl, StatusControlsLayout, ThumbnailControl, ThumbnailControlsLayout,
};
use purepic::ui::icon::Icon;
use purepic::ui::layout::{LayoutInput, RectF, ThumbnailDock, WindowLayout, compute_layout};
use purepic::ui::thumbnail::{
    THUMBNAIL_CACHE_BUDGET_BYTES, THUMBNAIL_CONTENT_DIP, THUMBNAIL_ITEM_EXTENT_DIP,
    THUMBNAIL_PANEL_PADDING_DIP, THUMBNAIL_QUEUE_CAPACITY, centered_scroll_offset,
    fit_thumbnail_overlay, max_scroll_offset, prioritized_thumbnail_indices,
    visible_prefetch_range,
};
use purepic::ui::zoom::{
    MAX_ZOOM, MIN_ZOOM, PointF, SizeF, fit_zoom, initial_zoom, origin_after_zoom, slider_to_zoom,
    step_zoom, wheel_zoom, zoom_to_slider,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{E_FAIL, HMODULE, HWND};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_IGNORE, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F,
    D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED, D2D1_FILL_MODE_WINDING, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_NONE,
    D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ELLIPSE, D2D1_FACTORY_OPTIONS,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_INTERPOLATION_MODE_LINEAR, D2D1_ROUNDED_RECT,
    D2D1CreateFactory, ID2D1Bitmap1, ID2D1Brush, ID2D1Device, ID2D1DeviceContext, ID2D1Factory1,
    ID2D1Image, ID2D1SolidColorBrush, ID2D1StrokeStyle,
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
const THUMBNAIL_BACKGROUND: D2D1_COLOR_F = color_alpha(0x27, 0x27, 0x27, 0.40);
const THUMBNAIL_PLACEHOLDER: D2D1_COLOR_F = color_alpha(0x2B, 0x2E, 0x30, 0.62);
const THUMBNAIL_HOVER: D2D1_COLOR_F = color(0x8B, 0x99, 0xA0);
const NAVIGATION_BACKGROUND: D2D1_COLOR_F = color_alpha(0x22, 0x25, 0x27, 0.88);
const NAVIGATION_HOVER: D2D1_COLOR_F = color_alpha(0x38, 0x3E, 0x41, 0.94);
const PRIMARY_TEXT: D2D1_COLOR_F = color(0xF4, 0xF6, 0xF8);
const SECONDARY_TEXT: D2D1_COLOR_F = color(0xB4, 0xBC, 0xC2);
const MUTED_TEXT: D2D1_COLOR_F = color(0x73, 0x7E, 0x85);
const CAPTION_HOVER: D2D1_COLOR_F = color(0x31, 0x3A, 0x3F);
const CAPTION_CLOSE_HOVER: D2D1_COLOR_F = color(0xC4, 0x2B, 0x1C);
const ACCENT: D2D1_COLOR_F = color(0x28, 0xD7, 0xE2);
const APP_TITLE: &str = "PurePic 图片查看器";
const TITLE_TEXT_LEFT_DIP: f32 = 176.0;
const STATUS_TEXT_LEFT_DIP: f32 = 128.0;
const STATUS_TEXT_RIGHT_RESERVED_DIP: f32 = 422.0;
const NAVIGATION_BUTTON_WIDTH_DIP: f32 = 36.0;
const NAVIGATION_BUTTON_HEIGHT_DIP: f32 = 64.0;
const NAVIGATION_EDGE_INSET_DIP: f32 = 16.0;
const NAVIGATION_PROXIMITY_X_DIP: f32 = 28.0;
const NAVIGATION_PROXIMITY_Y_DIP: f32 = 56.0;
const NAVIGATION_SHOW_DELAY: Duration = Duration::from_millis(120);
const NAVIGATION_FADE_IN_SECONDS: f32 = 0.14;
const NAVIGATION_FADE_OUT_SECONDS: f32 = 0.32;
const PAN_OVERFLOW_EPSILON_DIP: f32 = 0.5;
const WHEEL_DELTA: i32 = 120;

const fn color(r: u8, g: u8, b: u8) -> D2D1_COLOR_F {
    color_alpha(r, g, b, 1.0)
}

const fn color_alpha(r: u8, g: u8, b: u8, alpha: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: alpha,
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
    thumbnail_brush: ID2D1SolidColorBrush,
    thumbnail_placeholder_brush: ID2D1SolidColorBrush,
    thumbnail_hover_brush: ID2D1SolidColorBrush,
    navigation_brush: ID2D1SolidColorBrush,
    navigation_hover_brush: ID2D1SolidColorBrush,
    navigation_icon_brush: ID2D1SolidColorBrush,
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
    default_app_hot: bool,
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
    thumbnail_visible: bool,
    thumbnail_dock: ThumbnailDock,
    thumbnail_items: Vec<RenderedThumbnailItem>,
    thumbnail_selected: Option<usize>,
    thumbnail_hot: Option<usize>,
    thumbnail_control_hot: Option<ThumbnailControl>,
    dock_menu_open: bool,
    dock_menu_hot: Option<ThumbnailDock>,
    thumbnail_scroll: f32,
    thumbnail_cache_bytes: usize,
    thumbnail_cache_stamp: u64,
    thumbnail_scroll_drag: Option<ThumbnailScrollDrag>,
    navigation_target: Option<ImageNavigation>,
    navigation_displayed: Option<ImageNavigation>,
    navigation_hot: Option<ImageNavigation>,
    navigation_opacity: f32,
    navigation_target_since: Instant,
    navigation_last_tick: Instant,
    pan_x: f32,
    pan_y: f32,
    pan_last_position: Option<(f32, f32)>,
    zoom_wheel_remainder: i32,
}

struct RenderedImage {
    bitmap: ID2D1Bitmap1,
    width: u32,
    height: u32,
}

struct RenderedThumbnailItem {
    path: PathBuf,
    state: ThumbnailLoadState,
    image: Option<RenderedImage>,
    byte_size: usize,
    target_size_px: u32,
    last_used: u64,
}

#[derive(Clone, Copy, Debug)]
struct ThumbnailScrollDrag {
    pointer_origin: f32,
    scroll_origin: f32,
    track_extent: f32,
    thumb_extent: f32,
    maximum: f32,
}

#[derive(Clone, Copy, Debug)]
struct ThumbnailScrollbar {
    track: RectF,
    thumb: RectF,
    maximum: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ImageNavigation {
    Previous,
    Next,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ThumbnailLoadState {
    #[default]
    Empty,
    Queued,
    Ready,
    Failed,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PointerAction {
    #[default]
    None,
    BeginSlider,
    BeginThumbnailScroll,
    BeginPan,
    ToggleFullscreen,
    ToggleContextMenu,
    OpenDefaultAppSettings,
    ThumbnailPreferencesChanged,
    OpenThumbnail(usize),
}

#[derive(Clone, Debug)]
pub struct ThumbnailRequest {
    pub index: usize,
    pub path: PathBuf,
    pub target_size_px: u32,
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
        let thumbnail_brush =
            unsafe { context.CreateSolidColorBrush(&THUMBNAIL_BACKGROUND, None)? };
        let thumbnail_placeholder_brush =
            unsafe { context.CreateSolidColorBrush(&THUMBNAIL_PLACEHOLDER, None)? };
        let thumbnail_hover_brush =
            unsafe { context.CreateSolidColorBrush(&THUMBNAIL_HOVER, None)? };
        let navigation_brush =
            unsafe { context.CreateSolidColorBrush(&NAVIGATION_BACKGROUND, None)? };
        let navigation_hover_brush =
            unsafe { context.CreateSolidColorBrush(&NAVIGATION_HOVER, None)? };
        let navigation_icon_brush = unsafe { context.CreateSolidColorBrush(&PRIMARY_TEXT, None)? };
        let primary_text_brush = unsafe { context.CreateSolidColorBrush(&PRIMARY_TEXT, None)? };
        let secondary_text_brush = unsafe { context.CreateSolidColorBrush(&SECONDARY_TEXT, None)? };
        let muted_text_brush = unsafe { context.CreateSolidColorBrush(&MUTED_TEXT, None)? };
        let caption_hover_brush = unsafe { context.CreateSolidColorBrush(&CAPTION_HOVER, None)? };
        let caption_close_hover_brush =
            unsafe { context.CreateSolidColorBrush(&CAPTION_CLOSE_HOVER, None)? };
        let accent_brush = unsafe { context.CreateSolidColorBrush(&ACCENT, None)? };

        let now = Instant::now();
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
            thumbnail_brush,
            thumbnail_placeholder_brush,
            thumbnail_hover_brush,
            navigation_brush,
            navigation_hover_brush,
            navigation_icon_brush,
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
            default_app_hot: false,
            context_menu_registered: false,
            maximized: false,
            title: String::new(),
            status: "— × —     0 B".to_owned(),
            message: "打开图片以开始".to_owned(),
            image: None,
            icons: IconSet::load(),
            zoom: 1.0,
            fit_mode: true,
            zoom_menu_open: false,
            fullscreen: false,
            status_hot: None,
            zoom_menu_hot: None,
            thumbnail_visible: false,
            thumbnail_dock: ThumbnailDock::Bottom,
            thumbnail_items: Vec::new(),
            thumbnail_selected: None,
            thumbnail_hot: None,
            thumbnail_control_hot: None,
            dock_menu_open: false,
            dock_menu_hot: None,
            thumbnail_scroll: 0.0,
            thumbnail_cache_bytes: 0,
            thumbnail_cache_stamp: 0,
            thumbnail_scroll_drag: None,
            navigation_target: None,
            navigation_displayed: None,
            navigation_hot: None,
            navigation_opacity: 0.0,
            navigation_target_since: now,
            navigation_last_tick: now,
            pan_x: 0.0,
            pan_y: 0.0,
            pan_last_position: None,
            zoom_wheel_remainder: 0,
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
        let status_text_x = layout.status_bar.x + STATUS_TEXT_LEFT_DIP;
        let status_left = RectF::new(
            status_text_x,
            layout.status_bar.y,
            (layout.status_bar.right() - status_text_x - STATUS_TEXT_RIGHT_RESERVED_DIP).max(0.0),
            layout.status_bar.height,
        );
        let controls = StatusControlsLayout::compute(layout.status_bar);
        let thumbnail_controls = ThumbnailControlsLayout::compute(layout.status_bar);

        unsafe {
            self.context.BeginDraw();
            self.context.Clear(Some(&BACKGROUND));

            if let Some(image) = &self.image
                && let Some(destination) = self.image_destination(layout.canvas)
            {
                self.context.DrawBitmap(
                    &image.bitmap,
                    Some(&to_d2d_rect(destination)),
                    1.0,
                    D2D1_INTERPOLATION_MODE_LINEAR,
                    None,
                    None,
                );
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
            if !self.fullscreen
                && let Some(panel) = layout.thumbnail_panel
            {
                self.draw_thumbnails(panel);
            }
            self.draw_image_navigation(layout);
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
                self.draw_title_actions(layout.title_bar);
                self.draw_caption_buttons(layout.title_bar);
                self.draw_thumbnail_controls(thumbnail_controls);
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
                if self.dock_menu_open {
                    self.draw_dock_menu(thumbnail_controls.dock_menu);
                }
                self.draw_status_tooltip(controls, layout.canvas);
                self.draw_thumbnail_tooltip(thumbnail_controls, layout.canvas);
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
        let target_size_px = self.thumbnail_target_size_px();
        for item in &mut self.thumbnail_items {
            if item.state == ThumbnailLoadState::Queued
                || (item.image.is_some() && item.target_size_px != target_size_px)
            {
                item.state = ThumbnailLoadState::Empty;
            }
        }
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
        self.default_app_hot = false;
        self.thumbnail_hot = None;
        self.thumbnail_control_hot = None;
        self.dock_menu_open = false;
        self.dock_menu_hot = None;
        self.thumbnail_scroll_drag = None;
    }

    pub fn close_zoom_menu(&mut self) -> bool {
        self.zoom_menu_hot = None;
        self.dock_menu_hot = None;
        std::mem::take(&mut self.zoom_menu_open) | std::mem::take(&mut self.dock_menu_open)
    }

    pub fn set_pointer_hot(&mut self, x_px: i32, y_px: i32) -> bool {
        let (x, y) = self.point_to_dip(x_px, y_px);
        let layout = self.current_layout();
        let now = Instant::now();
        let animation_changed = self.advance_navigation_animation(now);
        let navigation_target = self.navigation_at_proximity(layout, x, y);
        let navigation_target_changed = self.navigation_target != navigation_target;
        if navigation_target_changed {
            self.navigation_target = navigation_target;
            self.navigation_target_since = now;
            if navigation_target.is_some() {
                self.navigation_displayed = navigation_target;
            }
        }
        let navigation_hot = navigation_target.filter(|direction| {
            self.navigation_button_rect(layout, *direction)
                .contains(x, y)
        });
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
        let default_app_hot =
            !self.fullscreen && default_app_button_rect(layout.title_bar).contains(x, y);
        let thumbnail_hot = if self.fullscreen {
            None
        } else {
            layout
                .thumbnail_panel
                .and_then(|panel| self.thumbnail_index_at(panel, x, y))
        };
        let thumbnail_control_hot = if self.fullscreen {
            None
        } else {
            ThumbnailControlsLayout::compute(layout.status_bar).hit_test(x, y)
        };
        let dock_menu_hot = if self.dock_menu_open && !self.fullscreen {
            dock_at(
                self.dock_menu_rect(ThumbnailControlsLayout::compute(layout.status_bar).dock_menu),
                x,
                y,
            )
        } else {
            None
        };
        if self.status_hot == status_hot
            && self.zoom_menu_hot == zoom_menu_hot
            && self.title_action_hot == title_action_hot
            && self.default_app_hot == default_app_hot
            && self.thumbnail_hot == thumbnail_hot
            && self.thumbnail_control_hot == thumbnail_control_hot
            && self.dock_menu_hot == dock_menu_hot
            && self.navigation_hot == navigation_hot
            && !navigation_target_changed
            && !animation_changed
        {
            return false;
        }
        self.status_hot = status_hot;
        self.zoom_menu_hot = zoom_menu_hot;
        self.title_action_hot = title_action_hot;
        self.default_app_hot = default_app_hot;
        self.thumbnail_hot = thumbnail_hot;
        self.thumbnail_control_hot = thumbnail_control_hot;
        self.dock_menu_hot = dock_menu_hot;
        self.navigation_hot = navigation_hot;
        true
    }

    pub fn clear_pointer_hot(&mut self) -> bool {
        let now = Instant::now();
        let animation_changed = self.advance_navigation_animation(now);
        let changed = self.status_hot.is_some()
            || self.zoom_menu_hot.is_some()
            || self.title_action_hot
            || self.default_app_hot
            || self.thumbnail_hot.is_some()
            || self.thumbnail_control_hot.is_some()
            || self.dock_menu_hot.is_some()
            || self.navigation_target.is_some()
            || self.navigation_hot.is_some()
            || animation_changed;
        self.status_hot = None;
        self.zoom_menu_hot = None;
        self.title_action_hot = false;
        self.default_app_hot = false;
        self.thumbnail_hot = None;
        self.thumbnail_control_hot = None;
        self.dock_menu_hot = None;
        self.navigation_target = None;
        self.navigation_hot = None;
        self.navigation_target_since = now;
        changed
    }

    pub fn tick_navigation_animation(&mut self) -> bool {
        self.advance_navigation_animation(Instant::now())
    }

    pub fn navigation_animation_active(&self) -> bool {
        let target_available = self
            .navigation_target
            .is_some_and(|direction| self.navigation_available(direction));
        if target_available {
            Instant::now().saturating_duration_since(self.navigation_target_since)
                < NAVIGATION_SHOW_DELAY
                || self.navigation_opacity < 1.0
        } else {
            self.navigation_opacity > 0.0
        }
    }

    pub fn pointer_down(&mut self, x_px: i32, y_px: i32) -> PointerAction {
        let (x, y) = self.point_to_dip(x_px, y_px);
        let layout = self.current_layout();
        if self.navigation_opacity >= 0.2
            && let Some(direction) = self.navigation_displayed
            && self.navigation_available(direction)
            && self
                .navigation_button_rect(layout, direction)
                .contains(x, y)
            && let Some(index) = self.navigation_index(direction)
        {
            return PointerAction::OpenThumbnail(index);
        }
        if self.fullscreen {
            return self.begin_pan(x, y, layout.canvas);
        }
        if default_app_button_rect(layout.title_bar).contains(x, y) {
            return PointerAction::OpenDefaultAppSettings;
        }
        if title_action_button_rect(layout.title_bar).contains(x, y) {
            return PointerAction::ToggleContextMenu;
        }
        let controls = StatusControlsLayout::compute(layout.status_bar);

        if self.dock_menu_open {
            if let Some(dock) = dock_at(
                self.dock_menu_rect(ThumbnailControlsLayout::compute(layout.status_bar).dock_menu),
                x,
                y,
            ) {
                self.thumbnail_dock = dock;
                self.thumbnail_scroll_drag = None;
                self.dock_menu_open = false;
                self.dock_menu_hot = None;
                self.center_selected_thumbnail();
                return PointerAction::ThumbnailPreferencesChanged;
            }
            self.dock_menu_open = false;
            self.dock_menu_hot = None;
            return PointerAction::None;
        }

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

        match ThumbnailControlsLayout::compute(layout.status_bar).hit_test(x, y) {
            Some(ThumbnailControl::Toggle) => {
                self.thumbnail_visible = !self.thumbnail_visible;
                self.thumbnail_scroll_drag = None;
                self.center_selected_thumbnail();
                return PointerAction::ThumbnailPreferencesChanged;
            }
            Some(ThumbnailControl::DockMenu) => {
                self.dock_menu_open = true;
                self.dock_menu_hot = None;
                return PointerAction::None;
            }
            None => {}
        }

        if let Some(panel) = layout.thumbnail_panel
            && self.begin_thumbnail_scroll(panel, x, y)
        {
            return PointerAction::BeginThumbnailScroll;
        }

        if let Some(index) = layout
            .thumbnail_panel
            .and_then(|panel| self.thumbnail_index_at(panel, x, y))
        {
            return PointerAction::OpenThumbnail(index);
        }
        if layout
            .thumbnail_panel
            .is_some_and(|panel| panel.contains(x, y))
        {
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

    pub fn pointer_move_thumbnail_scroll(&mut self, x_px: i32, y_px: i32) {
        let (x, y) = self.point_to_dip(x_px, y_px);
        let Some(drag) = self.thumbnail_scroll_drag else {
            return;
        };
        let pointer = if self.thumbnail_dock.is_horizontal() {
            x
        } else {
            y
        };
        let travel = (drag.track_extent - drag.thumb_extent).max(1.0);
        self.thumbnail_scroll = (drag.scroll_origin
            + (pointer - drag.pointer_origin) * drag.maximum / travel)
            .clamp(0.0, drag.maximum);
    }

    pub fn end_thumbnail_scroll(&mut self) {
        self.thumbnail_scroll_drag = None;
    }

    pub fn shows_pan_cursor(&self, x_px: i32, y_px: i32) -> bool {
        if self.fit_mode {
            return false;
        }
        let (x, y) = self.point_to_dip(x_px, y_px);
        let layout = self.current_layout();
        if layout
            .thumbnail_panel
            .is_some_and(|panel| panel.contains(x, y))
        {
            return false;
        }
        let canvas = layout.canvas;
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

    pub fn is_over_thumbnail_panel(&self, x_px: i32, y_px: i32) -> bool {
        let (x, y) = self.point_to_dip(x_px, y_px);
        self.current_layout()
            .thumbnail_panel
            .is_some_and(|panel| panel.contains(x, y))
    }

    pub fn set_thumbnail_catalog(&mut self, paths: Vec<PathBuf>, current_path: &Path) {
        self.thumbnail_items = paths
            .into_iter()
            .map(|path| RenderedThumbnailItem {
                path,
                state: ThumbnailLoadState::Empty,
                image: None,
                byte_size: 0,
                target_size_px: 0,
                last_used: 0,
            })
            .collect();
        self.thumbnail_selected = self
            .thumbnail_items
            .iter()
            .position(|item| paths_equal(&item.path, current_path));
        self.thumbnail_hot = None;
        self.thumbnail_cache_bytes = 0;
        self.thumbnail_cache_stamp = 0;
        self.navigation_target = None;
        self.navigation_displayed = None;
        self.navigation_hot = None;
        self.navigation_opacity = 0.0;
        self.center_selected_thumbnail();
    }

    pub fn set_thumbnail_preferences(&mut self, visible: bool, dock: ThumbnailDock) {
        self.thumbnail_visible = visible;
        self.thumbnail_dock = dock;
        self.center_selected_thumbnail();
    }

    pub fn thumbnail_preferences(&self) -> (bool, ThumbnailDock) {
        (self.thumbnail_visible, self.thumbnail_dock)
    }

    pub fn thumbnail_requests(&mut self) -> Vec<ThumbnailRequest> {
        let Some(panel) = self.current_layout().thumbnail_panel else {
            return Vec::new();
        };
        let viewport = self.thumbnail_viewport_extent(panel);
        self.thumbnail_scroll = self
            .thumbnail_scroll
            .clamp(0.0, max_scroll_offset(self.thumbnail_items.len(), viewport));
        let indices = prioritized_thumbnail_indices(
            self.thumbnail_items.len(),
            self.thumbnail_scroll,
            viewport,
            self.thumbnail_selected,
        );
        for item in &mut self.thumbnail_items {
            if item.state == ThumbnailLoadState::Queued {
                item.state = ThumbnailLoadState::Empty;
            }
        }
        self.thumbnail_cache_stamp = self.thumbnail_cache_stamp.wrapping_add(1);
        let stamp = self.thumbnail_cache_stamp;
        let target_size_px = self.thumbnail_target_size_px();
        let mut requests = Vec::new();
        for index in indices {
            let item = &mut self.thumbnail_items[index];
            if item.state == ThumbnailLoadState::Ready {
                item.last_used = stamp;
            } else if item.state == ThumbnailLoadState::Empty {
                requests.push(ThumbnailRequest {
                    index,
                    path: item.path.clone(),
                    target_size_px,
                });
                if requests.len() == THUMBNAIL_QUEUE_CAPACITY {
                    break;
                }
            }
        }
        requests
    }

    pub fn mark_thumbnail_queued(&mut self, index: usize, path: &Path) {
        if let Some(item) = self.thumbnail_items.get_mut(index)
            && item.state == ThumbnailLoadState::Empty
            && paths_equal(&item.path, path)
        {
            item.state = ThumbnailLoadState::Queued;
        }
    }

    pub fn set_thumbnail_image(
        &mut self,
        index: usize,
        path: &Path,
        target_size_px: u32,
        image: DecodedImage,
    ) -> Result<()> {
        let Some(item) = self.thumbnail_items.get(index) else {
            return Ok(());
        };
        if !paths_equal(&item.path, path) {
            return Ok(());
        }
        if target_size_px != self.thumbnail_target_size_px() {
            if item.state == ThumbnailLoadState::Queued {
                self.thumbnail_items[index].state = ThumbnailLoadState::Empty;
            }
            return Ok(());
        }
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
        let byte_size = image.pixels.len();
        self.thumbnail_cache_stamp = self.thumbnail_cache_stamp.wrapping_add(1);
        let item = &mut self.thumbnail_items[index];
        self.thumbnail_cache_bytes = self
            .thumbnail_cache_bytes
            .saturating_sub(item.byte_size)
            .saturating_add(byte_size);
        item.state = ThumbnailLoadState::Ready;
        item.image = Some(RenderedImage {
            bitmap,
            width: image.width,
            height: image.height,
        });
        item.byte_size = byte_size;
        item.target_size_px = target_size_px;
        item.last_used = self.thumbnail_cache_stamp;
        self.evict_thumbnail_cache();
        Ok(())
    }

    pub fn set_thumbnail_failed(&mut self, index: usize, path: &Path, target_size_px: u32) {
        if target_size_px != self.thumbnail_target_size_px() {
            if let Some(item) = self.thumbnail_items.get_mut(index)
                && item.state == ThumbnailLoadState::Queued
                && paths_equal(&item.path, path)
            {
                item.state = ThumbnailLoadState::Empty;
            }
            return;
        }
        if let Some(item) = self.thumbnail_items.get_mut(index)
            && paths_equal(&item.path, path)
        {
            if item.image.is_some() {
                item.state = ThumbnailLoadState::Ready;
            } else {
                self.thumbnail_cache_bytes =
                    self.thumbnail_cache_bytes.saturating_sub(item.byte_size);
                item.state = ThumbnailLoadState::Failed;
                item.image = None;
                item.byte_size = 0;
                item.target_size_px = 0;
            }
        }
    }

    pub fn thumbnail_path(&self, index: usize) -> Option<PathBuf> {
        self.thumbnail_items
            .get(index)
            .map(|item| item.path.clone())
    }

    pub fn adjacent_thumbnail_index(&self, direction: i32) -> Option<usize> {
        let selected = self.thumbnail_selected?;
        if direction < 0 {
            selected.checked_sub(1)
        } else if direction > 0 {
            (selected + 1 < self.thumbnail_items.len()).then_some(selected + 1)
        } else {
            None
        }
    }

    pub fn select_thumbnail(&mut self, index: usize) {
        if index < self.thumbnail_items.len() {
            self.thumbnail_selected = Some(index);
        }
    }

    pub fn scroll_thumbnails(&mut self, x_px: i32, y_px: i32, wheel_delta: i16) -> bool {
        let (x, y) = self.point_to_dip(x_px, y_px);
        let Some(panel) = self.current_layout().thumbnail_panel else {
            return false;
        };
        if !panel.contains(x, y) {
            return false;
        }
        let viewport = self.thumbnail_viewport_extent(panel);
        let maximum = max_scroll_offset(self.thumbnail_items.len(), viewport);
        let previous = self.thumbnail_scroll;
        self.thumbnail_scroll = (self.thumbnail_scroll
            - wheel_delta as f32 / 120.0 * THUMBNAIL_ITEM_EXTENT_DIP * 3.0)
            .clamp(0.0, maximum);
        (self.thumbnail_scroll - previous).abs() > f32::EPSILON
    }

    pub fn zoom_canvas_at(&mut self, x_px: i32, y_px: i32, wheel_delta: i16) -> bool {
        let (x, y) = self.point_to_dip(x_px, y_px);
        let layout = self.current_layout();
        if !layout.canvas.contains(x, y)
            || layout
                .thumbnail_panel
                .is_some_and(|panel| panel.contains(x, y))
            || self.image.is_none()
        {
            self.zoom_wheel_remainder = 0;
            return false;
        }

        let delta = i32::from(wheel_delta);
        if delta == 0 {
            return true;
        }
        if self.zoom_wheel_remainder != 0 && self.zoom_wheel_remainder.signum() != delta.signum() {
            self.zoom_wheel_remainder = 0;
        }
        self.zoom_wheel_remainder += delta;
        let steps = self.zoom_wheel_remainder / WHEEL_DELTA;
        self.zoom_wheel_remainder %= WHEEL_DELTA;
        if steps == 0 {
            return true;
        }

        let canvas = layout.canvas;
        let old_zoom = self.current_zoom(canvas);
        let Some(old_destination) = self.image_destination(canvas) else {
            return true;
        };
        let new_zoom = wheel_zoom(old_zoom as f64, steps) as f32;
        if (new_zoom - old_zoom).abs() <= f32::EPSILON {
            return true;
        }

        self.fit_mode = false;
        self.zoom = new_zoom;
        (self.pan_x, self.pan_y) = pan_after_anchored_zoom(
            canvas,
            old_destination,
            PointF::new(x as f64, y as f64),
            old_zoom,
            new_zoom,
        );
        self.constrain_pan();
        self.zoom_menu_open = false;
        self.zoom_menu_hot = None;
        true
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
        let canvas = self.current_layout().canvas;
        let dpi_scale = self.dpi as f64 / 96.0;
        let zoom = initial_zoom(
            SizeF::new(image.original_width as f64, image.original_height as f64),
            SizeF::new(
                canvas.width as f64 * dpi_scale,
                canvas.height as f64 * dpi_scale,
            ),
        ) as f32;
        self.image = Some(RenderedImage {
            bitmap,
            width: image.original_width,
            height: image.original_height,
        });
        self.zoom = zoom;
        self.fit_mode = zoom < 1.0;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
        self.pan_last_position = None;
        self.zoom_wheel_remainder = 0;
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

    unsafe fn draw_title_actions(&self, title_bar: RectF) {
        let default_button = default_app_button_rect(title_bar);
        if self.default_app_hot {
            let hover = centered_square(default_button, 36.0);
            unsafe {
                self.context.FillRoundedRectangle(
                    &to_d2d_rounded_rect(hover, 6.0),
                    &self.caption_hover_brush,
                )
            };
        }
        unsafe {
            draw_icon(
                &self.context,
                &self.d2d_factory,
                &self.icons.default_app,
                centered_square(default_button, 24.0),
                &self.primary_text_brush,
            );
        }

        let button = title_action_button_rect(title_bar);
        if self.title_action_hot {
            let hover = centered_square(button, 36.0);
            unsafe {
                self.context.FillRoundedRectangle(
                    &to_d2d_rounded_rect(hover, 6.0),
                    &self.caption_hover_brush,
                )
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
                centered_square(button, 24.0),
                &self.primary_text_brush,
            );
        }
    }

    unsafe fn draw_title_action_tooltip(&self, title_bar: RectF, canvas: RectF) {
        if !self.title_action_hot && !self.default_app_hot {
            return;
        }
        let (button, label, width) = if self.default_app_hot {
            (
                default_app_button_rect(title_bar),
                "设置为默认图片应用",
                140.0,
            )
        } else if self.context_menu_registered {
            (
                title_action_button_rect(title_bar),
                "取消图片右键菜单",
                132.0,
            )
        } else {
            (
                title_action_button_rect(title_bar),
                "注册图片右键菜单",
                132.0,
            )
        };
        let rect = RectF::new(
            (button.x + (button.width - width) * 0.5)
                .clamp(canvas.x + 8.0, canvas.right() - width - 8.0),
            title_bar.bottom() + 8.0,
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

    unsafe fn draw_thumbnails(&self, panel: RectF) {
        unsafe {
            self.context
                .FillRoundedRectangle(&to_d2d_rounded_rect(panel, 8.0), &self.thumbnail_brush);
            self.context
                .PushAxisAlignedClip(&to_d2d_rect(panel), D2D1_ANTIALIAS_MODE_ALIASED);
        }
        let viewport = self.thumbnail_viewport_extent(panel);
        let range =
            visible_prefetch_range(self.thumbnail_items.len(), self.thumbnail_scroll, viewport);
        for index in range {
            let cell = self.thumbnail_item_rect(panel, index);
            let selected = self.thumbnail_selected == Some(index);
            let hovered = self.thumbnail_hot == Some(index);
            let frame = centered_square(cell, THUMBNAIL_CONTENT_DIP + 4.0);
            let content = centered_square(cell, THUMBNAIL_CONTENT_DIP);
            unsafe {
                self.context.FillRoundedRectangle(
                    &to_d2d_rounded_rect(content, 4.0),
                    &self.thumbnail_placeholder_brush,
                );
            }
            if let Some(image) = self.thumbnail_items[index].image.as_ref() {
                let scale =
                    (content.width / image.width as f32).min(content.height / image.height as f32);
                let destination = RectF::new(
                    content.x + (content.width - image.width as f32 * scale) * 0.5,
                    content.y + (content.height - image.height as f32 * scale) * 0.5,
                    image.width as f32 * scale,
                    image.height as f32 * scale,
                );
                unsafe {
                    self.context.DrawBitmap(
                        &image.bitmap,
                        Some(&to_d2d_rect(destination)),
                        1.0,
                        D2D1_INTERPOLATION_MODE_LINEAR,
                        None,
                        None,
                    );
                }
            } else if self.thumbnail_items[index].state == ThumbnailLoadState::Failed {
                let inset = 30.0;
                let top_left = Vector2 {
                    X: content.x + inset,
                    Y: content.y + inset,
                };
                let top_right = Vector2 {
                    X: content.right() - inset,
                    Y: content.y + inset,
                };
                let bottom_left = Vector2 {
                    X: content.x + inset,
                    Y: content.bottom() - inset,
                };
                let bottom_right = Vector2 {
                    X: content.right() - inset,
                    Y: content.bottom() - inset,
                };
                unsafe {
                    self.context.DrawLine(
                        top_left,
                        bottom_right,
                        &self.muted_text_brush,
                        1.5,
                        None,
                    );
                    self.context.DrawLine(
                        top_right,
                        bottom_left,
                        &self.muted_text_brush,
                        1.5,
                        None,
                    );
                }
            }
            if selected {
                unsafe {
                    self.context.DrawRoundedRectangle(
                        &to_d2d_rounded_rect(frame, 5.0),
                        &self.accent_brush,
                        1.5,
                        None,
                    );
                }
            } else if hovered {
                unsafe {
                    self.context.DrawRoundedRectangle(
                        &to_d2d_rounded_rect(frame, 5.0),
                        &self.thumbnail_hover_brush,
                        1.25,
                        None,
                    );
                }
            }
        }
        unsafe { self.draw_thumbnail_scrollbar(panel) };
        unsafe { self.context.PopAxisAlignedClip() };
    }

    unsafe fn draw_image_navigation(&self, layout: WindowLayout) {
        if self.navigation_opacity <= 0.0 {
            return;
        }
        let Some(direction) = self.navigation_displayed else {
            return;
        };
        if !self.navigation_available(direction) {
            return;
        }
        let button = self.navigation_button_rect(layout, direction);
        let background = if self.navigation_hot == Some(direction) {
            &self.navigation_hover_brush
        } else {
            &self.navigation_brush
        };
        unsafe {
            background.SetOpacity(self.navigation_opacity);
            self.navigation_icon_brush
                .SetOpacity(self.navigation_opacity);
            self.context
                .FillRoundedRectangle(&to_d2d_rounded_rect(button, 7.0), background);
            draw_icon(
                &self.context,
                &self.d2d_factory,
                match direction {
                    ImageNavigation::Previous => &self.icons.image_previous,
                    ImageNavigation::Next => &self.icons.image_next,
                },
                centered_square(button, 22.0),
                &self.navigation_icon_brush,
            );
        }
    }

    unsafe fn draw_thumbnail_scrollbar(&self, panel: RectF) {
        let Some(scrollbar) = self.thumbnail_scrollbar(panel) else {
            return;
        };
        unsafe {
            self.context.FillRoundedRectangle(
                &to_d2d_rounded_rect(scrollbar.thumb, 1.5),
                &self.muted_text_brush,
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

    unsafe fn draw_thumbnail_controls(&self, controls: ThumbnailControlsLayout) {
        unsafe {
            if self.thumbnail_control_hot == Some(ThumbnailControl::Toggle) {
                self.context.FillRoundedRectangle(
                    &to_d2d_rounded_rect(controls.toggle, 6.0),
                    &self.caption_hover_brush,
                );
            }
            self.context.FillRoundedRectangle(
                &to_d2d_rounded_rect(controls.dock_menu, 6.0),
                &self.status_control_brush,
            );
            if self.thumbnail_control_hot == Some(ThumbnailControl::DockMenu) {
                self.context.FillRoundedRectangle(
                    &to_d2d_rounded_rect(controls.dock_menu, 6.0),
                    &self.caption_hover_brush,
                );
            }
            draw_icon(
                &self.context,
                &self.d2d_factory,
                &self.icons.thumbnails,
                centered_square(controls.toggle, 20.0),
                if self.thumbnail_visible {
                    &self.accent_brush
                } else {
                    &self.primary_text_brush
                },
            );
            let dock_content_x =
                controls.dock_menu.x + (controls.dock_menu.width - 32.0).max(0.0) * 0.5;
            let dock_icon_rect = RectF::new(dock_content_x, controls.dock_menu.y + 7.0, 18.0, 18.0);
            draw_icon(
                &self.context,
                &self.d2d_factory,
                self.dock_icon(self.thumbnail_dock),
                dock_icon_rect,
                &self.primary_text_brush,
            );
            let chevron = RectF::new(
                dock_content_x + 20.0,
                controls.dock_menu.y + (controls.dock_menu.height - 12.0) * 0.5,
                12.0,
                12.0,
            );
            draw_icon(
                &self.context,
                &self.d2d_factory,
                &self.icons.chevron_down,
                chevron,
                &self.primary_text_brush,
            );
            let separator = RectF::new(
                controls.dock_menu.right() + 12.0,
                controls.dock_menu.y + 4.0,
                1.0,
                controls.dock_menu.height - 8.0,
            );
            self.context
                .FillRectangle(&to_d2d_rect(separator), &self.muted_text_brush);
        }
    }

    unsafe fn draw_dock_menu(&self, button: RectF) {
        let menu = self.dock_menu_rect(button);
        unsafe {
            self.context
                .FillRoundedRectangle(&to_d2d_rounded_rect(menu, 8.0), &self.menu_brush);
        }
        for (index, dock) in DOCK_CHOICES.iter().copied().enumerate() {
            let row = RectF::new(menu.x, menu.y + index as f32 * 30.0, menu.width, 30.0);
            let brush = if dock == self.thumbnail_dock {
                Some(&self.menu_selected_brush)
            } else if self.dock_menu_hot == Some(dock) {
                Some(&self.menu_hover_brush)
            } else {
                None
            };
            if let Some(brush) = brush {
                unsafe {
                    self.context.FillRoundedRectangle(
                        &to_d2d_rounded_rect(
                            RectF::new(row.x + 3.0, row.y + 2.0, row.width - 6.0, 26.0),
                            5.0,
                        ),
                        brush,
                    );
                }
            }
            unsafe {
                draw_icon(
                    &self.context,
                    &self.d2d_factory,
                    self.dock_icon(dock),
                    RectF::new(row.x + 8.0, row.y + 6.0, 18.0, 18.0),
                    &self.primary_text_brush,
                );
                draw_text(
                    &self.context,
                    dock_label(dock),
                    &self.tooltip_format,
                    RectF::new(row.x + 28.0, row.y, row.width - 36.0, row.height),
                    &self.primary_text_brush,
                );
            }
        }
    }

    fn dock_icon(&self, dock: ThumbnailDock) -> &Icon {
        match dock {
            ThumbnailDock::Top => &self.icons.dock_top,
            ThumbnailDock::Bottom => &self.icons.dock_bottom,
            ThumbnailDock::Left => &self.icons.dock_left,
            ThumbnailDock::Right => &self.icons.dock_right,
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

    unsafe fn draw_thumbnail_tooltip(&self, controls: ThumbnailControlsLayout, canvas: RectF) {
        if self.dock_menu_open {
            return;
        }
        let Some(control) = self.thumbnail_control_hot else {
            return;
        };
        let (label, width) = control.tooltip();
        let button = match control {
            ThumbnailControl::Toggle => controls.toggle,
            ThumbnailControl::DockMenu => controls.dock_menu,
        };
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
        let mut input = LayoutInput::new(self.width_px, self.height_px, self.dpi);
        input.thumbnail_visible = self.thumbnail_visible && !self.thumbnail_items.is_empty();
        input.thumbnail_dock = self.thumbnail_dock;
        input.thumbnail_extent_dip = self.thumbnail_dock.default_extent_dip();
        let mut layout = compute_layout(input);
        if self.fullscreen {
            layout.canvas = layout.client;
            layout.thumbnail_panel = None;
        } else if let Some(panel) = layout.thumbnail_panel {
            layout.thumbnail_panel = Some(fit_thumbnail_overlay(
                panel,
                self.thumbnail_dock,
                self.thumbnail_items.len(),
            ));
        }
        layout
    }

    fn navigation_button_rect(&self, layout: WindowLayout, direction: ImageNavigation) -> RectF {
        let mut x = match direction {
            ImageNavigation::Previous => layout.canvas.x + NAVIGATION_EDGE_INSET_DIP,
            ImageNavigation::Next => {
                layout.canvas.right() - NAVIGATION_EDGE_INSET_DIP - NAVIGATION_BUTTON_WIDTH_DIP
            }
        };
        if let Some(panel) = layout.thumbnail_panel {
            match (direction, self.thumbnail_dock) {
                (ImageNavigation::Previous, ThumbnailDock::Left) => {
                    x = panel.right() + NAVIGATION_EDGE_INSET_DIP;
                }
                (ImageNavigation::Next, ThumbnailDock::Right) => {
                    x = panel.x - NAVIGATION_EDGE_INSET_DIP - NAVIGATION_BUTTON_WIDTH_DIP;
                }
                _ => {}
            }
        }
        RectF::new(
            x,
            layout.canvas.y + (layout.canvas.height - NAVIGATION_BUTTON_HEIGHT_DIP) * 0.5,
            NAVIGATION_BUTTON_WIDTH_DIP,
            NAVIGATION_BUTTON_HEIGHT_DIP,
        )
    }

    fn navigation_at_proximity(
        &self,
        layout: WindowLayout,
        x: f32,
        y: f32,
    ) -> Option<ImageNavigation> {
        if layout
            .thumbnail_panel
            .is_some_and(|panel| panel.contains(x, y))
        {
            return None;
        }
        [ImageNavigation::Previous, ImageNavigation::Next]
            .into_iter()
            .find(|direction| {
                if !self.navigation_available(*direction) {
                    return false;
                }
                let button = self.navigation_button_rect(layout, *direction);
                RectF::new(
                    button.x - NAVIGATION_PROXIMITY_X_DIP,
                    button.y - NAVIGATION_PROXIMITY_Y_DIP,
                    button.width + NAVIGATION_PROXIMITY_X_DIP * 2.0,
                    button.height + NAVIGATION_PROXIMITY_Y_DIP * 2.0,
                )
                .contains(x, y)
            })
    }

    fn navigation_available(&self, direction: ImageNavigation) -> bool {
        let Some(selected) = self.thumbnail_selected else {
            return false;
        };
        match direction {
            ImageNavigation::Previous => selected > 0,
            ImageNavigation::Next => selected + 1 < self.thumbnail_items.len(),
        }
    }

    fn navigation_index(&self, direction: ImageNavigation) -> Option<usize> {
        let selected = self.thumbnail_selected?;
        match direction {
            ImageNavigation::Previous => selected.checked_sub(1),
            ImageNavigation::Next => {
                (selected + 1 < self.thumbnail_items.len()).then_some(selected + 1)
            }
        }
    }

    fn advance_navigation_animation(&mut self, now: Instant) -> bool {
        let elapsed = now
            .saturating_duration_since(self.navigation_last_tick)
            .as_secs_f32();
        self.navigation_last_tick = now;
        let should_show = self
            .navigation_target
            .is_some_and(|direction| self.navigation_available(direction))
            && now.saturating_duration_since(self.navigation_target_since) >= NAVIGATION_SHOW_DELAY;
        let previous = self.navigation_opacity;
        self.navigation_opacity =
            navigation_opacity_after(self.navigation_opacity, elapsed, should_show);
        if self.navigation_opacity <= 0.0 && self.navigation_target.is_none() {
            self.navigation_displayed = None;
        }
        (self.navigation_opacity - previous).abs() > f32::EPSILON
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
        if self.fit_mode || !canvas.contains(x, y) {
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

    fn thumbnail_viewport_extent(&self, panel: RectF) -> f32 {
        if self.thumbnail_dock.is_horizontal() {
            (panel.width - THUMBNAIL_PANEL_PADDING_DIP * 2.0).max(0.0)
        } else {
            (panel.height - THUMBNAIL_PANEL_PADDING_DIP * 2.0).max(0.0)
        }
    }

    fn thumbnail_item_rect(&self, panel: RectF, index: usize) -> RectF {
        let position = index as f32 * THUMBNAIL_ITEM_EXTENT_DIP - self.thumbnail_scroll;
        if self.thumbnail_dock.is_horizontal() {
            RectF::new(
                panel.x + THUMBNAIL_PANEL_PADDING_DIP + position,
                panel.y,
                THUMBNAIL_ITEM_EXTENT_DIP,
                panel.height,
            )
        } else {
            RectF::new(
                panel.x,
                panel.y + THUMBNAIL_PANEL_PADDING_DIP + position,
                panel.width,
                THUMBNAIL_ITEM_EXTENT_DIP,
            )
        }
    }

    fn thumbnail_index_at(&self, panel: RectF, x: f32, y: f32) -> Option<usize> {
        if !panel.contains(x, y) {
            return None;
        }
        let axis_position = if self.thumbnail_dock.is_horizontal() {
            x - panel.x - THUMBNAIL_PANEL_PADDING_DIP
        } else {
            y - panel.y - THUMBNAIL_PANEL_PADDING_DIP
        };
        let viewport = self.thumbnail_viewport_extent(panel);
        if axis_position < 0.0 || axis_position >= viewport {
            return None;
        }
        let position = axis_position + self.thumbnail_scroll;
        let index = (position / THUMBNAIL_ITEM_EXTENT_DIP).floor().max(0.0) as usize;
        (index < self.thumbnail_items.len()).then_some(index)
    }

    fn center_selected_thumbnail(&mut self) {
        let Some(index) = self.thumbnail_selected else {
            self.thumbnail_scroll = 0.0;
            return;
        };
        let Some(panel) = self.current_layout().thumbnail_panel else {
            return;
        };
        self.thumbnail_scroll = centered_scroll_offset(
            index,
            self.thumbnail_items.len(),
            self.thumbnail_viewport_extent(panel),
        );
    }

    fn thumbnail_scrollbar(&self, panel: RectF) -> Option<ThumbnailScrollbar> {
        let viewport = self.thumbnail_viewport_extent(panel);
        let content = self.thumbnail_items.len() as f32 * THUMBNAIL_ITEM_EXTENT_DIP;
        let maximum = max_scroll_offset(self.thumbnail_items.len(), viewport);
        if maximum <= 0.0 || content <= 0.0 {
            return None;
        }
        let horizontal = self.thumbnail_dock.is_horizontal();
        let track = if horizontal {
            RectF::new(panel.x + 8.0, panel.bottom() - 6.0, panel.width - 16.0, 3.0)
        } else {
            RectF::new(panel.right() - 6.0, panel.y + 8.0, 3.0, panel.height - 16.0)
        };
        let track_extent = if horizontal {
            track.width
        } else {
            track.height
        };
        let thumb_extent = (track_extent * viewport / content)
            .max(16.0)
            .min(track_extent);
        let thumb_offset = (track_extent - thumb_extent) * self.thumbnail_scroll / maximum;
        let thumb = if horizontal {
            RectF::new(track.x + thumb_offset, track.y, thumb_extent, track.height)
        } else {
            RectF::new(track.x, track.y + thumb_offset, track.width, thumb_extent)
        };
        Some(ThumbnailScrollbar {
            track,
            thumb,
            maximum,
        })
    }

    fn begin_thumbnail_scroll(&mut self, panel: RectF, x: f32, y: f32) -> bool {
        let Some(mut scrollbar) = self.thumbnail_scrollbar(panel) else {
            return false;
        };
        let horizontal = self.thumbnail_dock.is_horizontal();
        let hit_rect = if horizontal {
            RectF::new(panel.x, panel.bottom() - 12.0, panel.width, 12.0)
        } else {
            RectF::new(panel.right() - 12.0, panel.y, 12.0, panel.height)
        };
        if !hit_rect.contains(x, y) {
            return false;
        }
        let pointer = if horizontal { x } else { y };
        let thumb_start = if horizontal {
            scrollbar.thumb.x
        } else {
            scrollbar.thumb.y
        };
        let thumb_end = if horizontal {
            scrollbar.thumb.right()
        } else {
            scrollbar.thumb.bottom()
        };
        if pointer < thumb_start || pointer >= thumb_end {
            let track_start = if horizontal {
                scrollbar.track.x
            } else {
                scrollbar.track.y
            };
            let track_extent = if horizontal {
                scrollbar.track.width
            } else {
                scrollbar.track.height
            };
            let thumb_extent = if horizontal {
                scrollbar.thumb.width
            } else {
                scrollbar.thumb.height
            };
            let travel = (track_extent - thumb_extent).max(1.0);
            self.thumbnail_scroll = ((pointer - track_start - thumb_extent * 0.5) / travel
                * scrollbar.maximum)
                .clamp(0.0, scrollbar.maximum);
            scrollbar = self
                .thumbnail_scrollbar(panel)
                .expect("scrollbar disappeared");
        }
        self.thumbnail_scroll_drag = Some(ThumbnailScrollDrag {
            pointer_origin: pointer,
            scroll_origin: self.thumbnail_scroll,
            track_extent: if horizontal {
                scrollbar.track.width
            } else {
                scrollbar.track.height
            },
            thumb_extent: if horizontal {
                scrollbar.thumb.width
            } else {
                scrollbar.thumb.height
            },
            maximum: scrollbar.maximum,
        });
        true
    }

    fn evict_thumbnail_cache(&mut self) {
        while self.thumbnail_cache_bytes > THUMBNAIL_CACHE_BUDGET_BYTES {
            let candidate = self
                .thumbnail_items
                .iter()
                .enumerate()
                .filter(|(index, item)| {
                    item.image.is_some() && Some(*index) != self.thumbnail_selected
                })
                .min_by_key(|(_, item)| item.last_used)
                .map(|(index, _)| index);
            let Some(index) = candidate else {
                break;
            };
            let item = &mut self.thumbnail_items[index];
            self.thumbnail_cache_bytes = self.thumbnail_cache_bytes.saturating_sub(item.byte_size);
            if item.state == ThumbnailLoadState::Ready {
                item.state = ThumbnailLoadState::Empty;
            }
            item.image = None;
            item.byte_size = 0;
            item.target_size_px = 0;
        }
    }

    fn thumbnail_target_size_px(&self) -> u32 {
        (THUMBNAIL_CONTENT_DIP * self.dpi as f32 / 96.0)
            .ceil()
            .max(1.0) as u32
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
        RectF::new(button.x, button.y - height - 6.0, 88.0, height)
    }

    fn dock_menu_rect(&self, button: RectF) -> RectF {
        let height = DOCK_CHOICES.len() as f32 * 30.0;
        RectF::new(button.x, button.y - height - 6.0, 80.0, height)
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

const DOCK_CHOICES: [ThumbnailDock; 4] = [
    ThumbnailDock::Top,
    ThumbnailDock::Bottom,
    ThumbnailDock::Left,
    ThumbnailDock::Right,
];

fn dock_label(dock: ThumbnailDock) -> &'static str {
    match dock {
        ThumbnailDock::Top => "上方",
        ThumbnailDock::Bottom => "下方",
        ThumbnailDock::Left => "左侧",
        ThumbnailDock::Right => "右侧",
    }
}

fn dock_at(menu: RectF, x: f32, y: f32) -> Option<ThumbnailDock> {
    if !menu.contains(x, y) {
        return None;
    }
    let index = ((y - menu.y) / 30.0).floor() as usize;
    DOCK_CHOICES.get(index).copied()
}

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
    let right = default_app_button_rect(title_bar).x - 8.0;
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
    if image_extent <= canvas_extent + PAN_OVERFLOW_EPSILON_DIP {
        return 0.0;
    }
    let limit = (image_extent - canvas_extent) * 0.5;
    pan.clamp(-limit, limit)
}

fn pan_after_anchored_zoom(
    canvas: RectF,
    old_destination: RectF,
    anchor: PointF,
    old_zoom: f32,
    new_zoom: f32,
) -> (f32, f32) {
    let new_origin = origin_after_zoom(
        PointF::new(old_destination.x as f64, old_destination.y as f64),
        anchor,
        old_zoom as f64,
        new_zoom as f64,
    );
    let ratio = new_zoom / old_zoom;
    let new_width = old_destination.width * ratio;
    let new_height = old_destination.height * ratio;
    (
        new_origin.x as f32 - (canvas.x + (canvas.width - new_width) * 0.5),
        new_origin.y as f32 - (canvas.y + (canvas.height - new_height) * 0.5),
    )
}

fn image_exceeds_canvas(canvas: RectF, width: f32, height: f32) -> bool {
    width > canvas.width + PAN_OVERFLOW_EPSILON_DIP
        || height > canvas.height + PAN_OVERFLOW_EPSILON_DIP
}

fn navigation_opacity_after(current: f32, elapsed_seconds: f32, showing: bool) -> f32 {
    if showing {
        (current + elapsed_seconds / NAVIGATION_FADE_IN_SECONDS).min(1.0)
    } else {
        (current - elapsed_seconds / NAVIGATION_FADE_OUT_SECONDS).max(0.0)
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

fn paths_equal(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
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
        assert_eq!(constrain_pan_axis(50.0, 800.25, 800.0), 0.0);
    }

    #[test]
    fn image_is_pannable_when_either_axis_exceeds_the_canvas() {
        let canvas = RectF::new(0.0, 0.0, 800.0, 600.0);
        assert!(image_exceeds_canvas(canvas, 801.0, 500.0));
        assert!(image_exceeds_canvas(canvas, 700.0, 601.0));
        assert!(!image_exceeds_canvas(canvas, 800.0, 600.0));
        assert!(!image_exceeds_canvas(canvas, 800.25, 600.25));
    }

    #[test]
    fn anchored_zoom_preserves_the_image_point_under_the_pointer() {
        let canvas = RectF::new(0.0, 0.0, 800.0, 600.0);
        let old_destination = RectF::new(0.0, 0.0, 800.0, 600.0);
        let anchor = PointF::new(200.0, 150.0);
        let (pan_x, pan_y) = pan_after_anchored_zoom(canvas, old_destination, anchor, 1.0, 2.0);

        assert_eq!((pan_x, pan_y), (200.0, 150.0));
        let new_origin = PointF::new(-400.0 + pan_x as f64, -300.0 + pan_y as f64);
        assert_eq!(
            PointF::new(
                (anchor.x - new_origin.x) / 2.0,
                (anchor.y - new_origin.y) / 2.0,
            ),
            PointF::new(200.0, 150.0)
        );
    }

    #[test]
    fn navigation_buttons_fade_in_faster_than_they_fade_out() {
        assert_eq!(navigation_opacity_after(0.0, 0.07, true), 0.5);
        assert_eq!(navigation_opacity_after(1.0, 0.16, false), 0.5);
        assert_eq!(navigation_opacity_after(0.9, 1.0, true), 1.0);
        assert_eq!(navigation_opacity_after(0.1, 1.0, false), 0.0);
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
        assert_eq!(text.right(), 660.0);

        let narrow = title_text_rect(RectF::new(0.0, 0.0, 300.0, 44.0));
        assert_eq!(narrow.width, 0.0);
    }
}
