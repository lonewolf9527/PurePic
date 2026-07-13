use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;

use windows::Win32::Foundation::{E_FAIL, GENERIC_READ};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory2, GUID_WICPixelFormat32bppPBGRA, IWICBitmapSource, IWICImagingFactory,
    IWICPalette, WICBitmapDitherTypeNone, WICBitmapInterpolationModeFant,
    WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnLoad,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::core::{Error, Interface, PCWSTR, Result};

#[derive(Debug)]
pub struct DecodedImage {
    pub file_name: String,
    pub original_width: u32,
    pub original_height: u32,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub file_size: u64,
    pub pixels: Vec<u8>,
}

pub fn decode_preview(path: &Path, target_width: u32, target_height: u32) -> Result<DecodedImage> {
    let com = ComApartment::initialize()?;
    let result = decode_preview_inner(path, target_width, target_height);
    drop(com);
    result
}

fn decode_preview_inner(
    path: &Path,
    target_width: u32,
    target_height: u32,
) -> Result<DecodedImage> {
    let factory: IWICImagingFactory = unsafe {
        CoCreateInstance(
            &CLSID_WICImagingFactory2,
            None::<&windows::core::IUnknown>,
            CLSCTX_INPROC_SERVER,
        )?
    };
    let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let decoder = unsafe {
        factory.CreateDecoderFromFilename(
            PCWSTR(wide_path.as_ptr()),
            None,
            GENERIC_READ,
            WICDecodeMetadataCacheOnLoad,
        )?
    };
    let frame = unsafe { decoder.GetFrame(0)? };

    let mut original_width = 0;
    let mut original_height = 0;
    unsafe { frame.GetSize(&mut original_width, &mut original_height)? };
    if original_width == 0 || original_height == 0 {
        return Err(Error::new(E_FAIL, "The image has invalid dimensions"));
    }

    let (width, height) = preview_dimensions(
        original_width,
        original_height,
        target_width.max(1),
        target_height.max(1),
    );
    let frame_source: IWICBitmapSource = frame.cast()?;
    let source = if width != original_width || height != original_height {
        let scaler = unsafe { factory.CreateBitmapScaler()? };
        unsafe {
            scaler.Initialize(&frame_source, width, height, WICBitmapInterpolationModeFant)?;
        }
        scaler.cast::<IWICBitmapSource>()?
    } else {
        frame_source
    };

    let converter = unsafe { factory.CreateFormatConverter()? };
    unsafe {
        converter.Initialize(
            &source,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapDitherTypeNone,
            None::<&IWICPalette>,
            0.0,
            WICBitmapPaletteTypeCustom,
        )?;
    }

    let stride = width
        .checked_mul(4)
        .ok_or_else(|| Error::new(E_FAIL, "The image row is too large"))?;
    let buffer_size = stride
        .checked_mul(height)
        .and_then(|size| usize::try_from(size).ok())
        .ok_or_else(|| Error::new(E_FAIL, "The decoded image is too large"))?;
    let mut pixels = vec![0; buffer_size];
    unsafe {
        converter.CopyPixels(ptr::null(), stride, &mut pixels)?;
    }

    let file_size = fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());

    Ok(DecodedImage {
        file_name,
        original_width,
        original_height,
        width,
        height,
        stride,
        file_size,
        pixels,
    })
}

fn preview_dimensions(
    width: u32,
    height: u32,
    target_width: u32,
    target_height: u32,
) -> (u32, u32) {
    if width <= target_width && height <= target_height {
        return (width, height);
    }

    let scale = (target_width as f64 / width as f64)
        .min(target_height as f64 / height as f64)
        .min(1.0);
    let scaled_width = (width as f64 * scale).round().max(1.0) as u32;
    let scaled_height = (height as f64 * scale).round().max(1.0) as u32;
    (scaled_width, scaled_height)
}

struct ComApartment;

impl ComApartment {
    fn initialize() -> Result<Self> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

#[cfg(test)]
mod tests {
    use super::preview_dimensions;

    #[test]
    fn preview_keeps_small_images_at_original_size() {
        assert_eq!(preview_dimensions(800, 600, 1920, 1080), (800, 600));
    }

    #[test]
    fn preview_preserves_aspect_ratio() {
        assert_eq!(preview_dimensions(3840, 2160, 1280, 700), (1244, 700));
        assert_eq!(preview_dimensions(2160, 3840, 1280, 700), (394, 700));
    }
}
