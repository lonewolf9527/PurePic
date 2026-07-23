use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{E_ABORT, E_FAIL, HWND};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize, IBindCtx,
};
use windows::Win32::UI::Shell::{
    FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FOFX_RECYCLEONDELETE,
    FileOperation, IFileOperation, IFileOperationProgressSink, IShellItem,
    SHCreateItemFromParsingName,
};
use windows::core::{Error, IUnknown, PCWSTR, Result};

pub fn move_file_to_recycle_bin(owner: HWND, path: &Path) -> Result<()> {
    let absolute = absolute_path(path)?;
    if !absolute.is_file() {
        return Err(Error::new(E_FAIL, "图片文件不存在"));
    }

    let _apartment = ComApartment::initialize()?;
    let wide_path: Vec<u16> = absolute.as_os_str().encode_wide().chain(Some(0)).collect();
    let item: IShellItem =
        unsafe { SHCreateItemFromParsingName(PCWSTR(wide_path.as_ptr()), None::<&IBindCtx>)? };
    let operation: IFileOperation =
        unsafe { CoCreateInstance(&FileOperation, None::<&IUnknown>, CLSCTX_INPROC_SERVER)? };
    unsafe {
        operation.SetOwnerWindow(owner)?;
        operation.SetOperationFlags(
            FOF_ALLOWUNDO | FOFX_RECYCLEONDELETE | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT,
        )?;
        operation.DeleteItem(&item, None::<&IFileOperationProgressSink>)?;
        operation.PerformOperations()?;
        if operation.GetAnyOperationsAborted()?.as_bool() {
            return Err(Error::new(E_ABORT, "回收操作已取消"));
        }
    }
    if absolute.exists() {
        return Err(Error::new(E_FAIL, "图片未能移入回收站"));
    }
    Ok(())
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

struct ComApartment;

impl ComApartment {
    fn initialize() -> Result<Self> {
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
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
    use super::*;

    #[test]
    fn relative_paths_are_made_absolute_without_touching_the_file() {
        let path = absolute_path(Path::new("images\\photo.png")).unwrap();
        assert!(path.is_absolute());
        assert!(path.ends_with(r"images\photo.png"));
    }
}
