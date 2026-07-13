use purepic::ui::layout::{LayoutInput, RectF, compute_layout};
use windows::Win32::Foundation::{E_FAIL, HMODULE, HWND};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1,
    D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_FACTORY_OPTIONS,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1CreateFactory, ID2D1Bitmap1, ID2D1Device,
    ID2D1DeviceContext, ID2D1Factory1, ID2D1Image, ID2D1SolidColorBrush,
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

const BACKGROUND: D2D1_COLOR_F = color(0x0F, 0x14, 0x17);
const TITLE_BACKGROUND: D2D1_COLOR_F = color(0x18, 0x20, 0x24);
const STATUS_BACKGROUND: D2D1_COLOR_F = color(0x1B, 0x23, 0x27);
const PRIMARY_TEXT: D2D1_COLOR_F = color(0xF4, 0xF6, 0xF8);
const SECONDARY_TEXT: D2D1_COLOR_F = color(0xB4, 0xBC, 0xC2);
const MUTED_TEXT: D2D1_COLOR_F = color(0x73, 0x7E, 0x85);

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
    title_format: IDWriteTextFormat,
    status_format: IDWriteTextFormat,
    message_format: IDWriteTextFormat,
    dpi: u32,
    width_px: u32,
    height_px: u32,
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
            title_format,
            status_format,
            message_format,
            dpi: dpi.max(1),
            width_px,
            height_px,
        };
        renderer.create_target()?;
        Ok(renderer)
    }

    pub fn render(&self) -> Result<()> {
        if self.width_px == 0 || self.height_px == 0 || self.target.is_none() {
            return Ok(());
        }

        let layout = compute_layout(LayoutInput::new(self.width_px, self.height_px, self.dpi));
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
        let status_right = RectF::new(
            (layout.status_bar.right() - 420.0).max(0.0),
            layout.status_bar.y,
            layout.status_bar.width.min(400.0),
            layout.status_bar.height,
        );

        unsafe {
            self.context.BeginDraw();
            self.context.Clear(Some(&BACKGROUND));
            self.context
                .FillRectangle(&to_d2d_rect(layout.title_bar), &self.title_brush);
            self.context
                .FillRectangle(&to_d2d_rect(layout.status_bar), &self.status_brush);

            draw_text(
                &self.context,
                "PurePic",
                &self.title_format,
                layout.title_bar,
                &self.primary_text_brush,
            );
            draw_text(
                &self.context,
                "Open an image to begin",
                &self.message_format,
                canvas_center,
                &self.muted_text_brush,
            );
            draw_text(
                &self.context,
                "— × —     0 B",
                &self.status_format,
                status_left,
                &self.secondary_text_brush,
            );
            draw_text(
                &self.context,
                "1:1    100%    −   ━━●━━   +    ⛶",
                &self.title_format,
                status_right,
                &self.primary_text_brush,
            );

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
