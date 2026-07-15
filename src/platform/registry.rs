use std::path::Path;

use purepic::ui::layout::ThumbnailDock;
use purepic::ui::thumbnail::SUPPORTED_IMAGE_EXTENSIONS;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ,
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{PCWSTR, Result, w};

const SETTINGS_KEY: &str = r"Software\PurePic";
const CONTEXT_MENU_KEY: &str = r"Software\Classes\SystemFileAssociations\image\shell\PurePic";
const CONTEXT_MENU_COMMAND_KEY: &str =
    r"Software\Classes\SystemFileAssociations\image\shell\PurePic\command";
const APP_NAME: &str = "PurePic";
const PROG_ID: &str = "PurePic.Image";
const PROG_ID_KEY: &str = r"Software\Classes\PurePic.Image";
const CAPABILITIES_KEY: &str = r"Software\PurePic\Capabilities";
const FILE_ASSOCIATIONS_KEY: &str = r"Software\PurePic\Capabilities\FileAssociations";
const REGISTERED_APPLICATIONS_KEY: &str = r"Software\RegisteredApplications";
const APPLICATION_KEY: &str = r"Software\Classes\Applications\PurePic.exe";
const APPLICATION_COMMAND_KEY: &str =
    r"Software\Classes\Applications\PurePic.exe\shell\open\command";
const APPLICATION_SUPPORTED_TYPES_KEY: &str =
    r"Software\Classes\Applications\PurePic.exe\SupportedTypes";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SavedWindowState {
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThumbnailPreferences {
    pub visible: bool,
    pub dock: ThumbnailDock,
}

impl Default for ThumbnailPreferences {
    fn default() -> Self {
        Self {
            visible: false,
            dock: ThumbnailDock::Bottom,
        }
    }
}

struct OwnedKey(HKEY);

impl Drop for OwnedKey {
    fn drop(&mut self) {
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

pub fn load_window_state() -> Option<SavedWindowState> {
    let key = open_key(SETTINGS_KEY, KEY_READ).ok()?;
    Some(SavedWindowState {
        width: query_dword(&key, "WindowWidth")?,
        height: query_dword(&key, "WindowHeight")?,
        maximized: query_dword(&key, "WindowMaximized")? != 0,
    })
}

pub fn save_window_state(state: SavedWindowState) -> Result<()> {
    let key = create_key(SETTINGS_KEY)?;
    set_dword(&key, "WindowWidth", state.width)?;
    set_dword(&key, "WindowHeight", state.height)?;
    set_dword(&key, "WindowMaximized", u32::from(state.maximized))
}

pub fn load_thumbnail_preferences() -> ThumbnailPreferences {
    let Ok(key) = open_key(SETTINGS_KEY, KEY_READ) else {
        return ThumbnailPreferences::default();
    };
    ThumbnailPreferences {
        visible: query_dword(&key, "ThumbnailVisible").unwrap_or(0) != 0,
        dock: dword_to_dock(query_dword(&key, "ThumbnailDock").unwrap_or(1)),
    }
}

pub fn save_thumbnail_preferences(preferences: ThumbnailPreferences) -> Result<()> {
    let key = create_key(SETTINGS_KEY)?;
    set_dword(&key, "ThumbnailVisible", u32::from(preferences.visible))?;
    set_dword(&key, "ThumbnailDock", dock_to_dword(preferences.dock))
}

pub fn is_context_menu_registered() -> bool {
    open_key(CONTEXT_MENU_COMMAND_KEY, KEY_READ).is_ok()
}

pub fn set_context_menu_registered(registered: bool) -> Result<()> {
    if !registered {
        let path = wide(CONTEXT_MENU_KEY);
        let status = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr())) };
        if status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND {
            return Ok(());
        }
        return status.ok();
    }

    let executable = std::env::current_exe()?;
    let result = register_context_menu(&executable);
    if result.is_err() {
        let path = wide(CONTEXT_MENU_KEY);
        let _ = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr())) };
    }
    result
}

pub fn register_default_app_and_open_settings() -> Result<()> {
    let executable = std::env::current_exe()?;
    register_default_app(&executable)?;
    unsafe {
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
        let _ = ShellExecuteW(
            None,
            w!("open"),
            w!("ms-settings:defaultapps?registeredAppUser=PurePic"),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
    Ok(())
}

fn register_context_menu(executable: &Path) -> Result<()> {
    let shell_key = create_key(CONTEXT_MENU_KEY)?;
    set_string(&shell_key, None, "使用 PurePic 打开")?;
    set_string(
        &shell_key,
        Some("Icon"),
        &format!("\"{}\",0", executable.display()),
    )?;
    set_string(&shell_key, Some("MultiSelectModel"), "Single")?;

    let command_key = create_key(CONTEXT_MENU_COMMAND_KEY)?;
    set_string(&command_key, None, &context_menu_command(executable))
}

fn register_default_app(executable: &Path) -> Result<()> {
    let icon = format!("\"{}\",0", executable.display());
    let command = context_menu_command(executable);

    let prog_id = create_key(PROG_ID_KEY)?;
    set_string(&prog_id, None, "PurePic 图片")?;
    set_string(&prog_id, Some("FriendlyTypeName"), "PurePic 图片")?;

    let default_icon = create_key(&format!(r"{PROG_ID_KEY}\DefaultIcon"))?;
    set_string(&default_icon, None, &icon)?;
    let prog_id_command = create_key(&format!(r"{PROG_ID_KEY}\shell\open\command"))?;
    set_string(&prog_id_command, None, &command)?;

    let application = create_key(APPLICATION_KEY)?;
    set_string(&application, Some("FriendlyAppName"), APP_NAME)?;
    let application_command = create_key(APPLICATION_COMMAND_KEY)?;
    set_string(&application_command, None, &command)?;
    let supported_types = create_key(APPLICATION_SUPPORTED_TYPES_KEY)?;

    let capabilities = create_key(CAPABILITIES_KEY)?;
    set_string(&capabilities, Some("ApplicationName"), APP_NAME)?;
    set_string(
        &capabilities,
        Some("ApplicationDescription"),
        "快速、原生的 Windows 图片查看器",
    )?;
    set_string(&capabilities, Some("ApplicationIcon"), &icon)?;
    let associations = create_key(FILE_ASSOCIATIONS_KEY)?;
    for extension in SUPPORTED_IMAGE_EXTENSIONS {
        let extension = format!(".{extension}");
        set_string(&associations, Some(&extension), PROG_ID)?;
        set_string(&supported_types, Some(&extension), "")?;
    }

    let registered_applications = create_key(REGISTERED_APPLICATIONS_KEY)?;
    set_string(&registered_applications, Some(APP_NAME), CAPABILITIES_KEY)
}

fn context_menu_command(executable: &Path) -> String {
    format!("\"{}\" \"%1\"", executable.display())
}

fn create_key(path: &str) -> Result<OwnedKey> {
    let path = wide(path);
    let mut key = HKEY::default();
    unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(path.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut key,
            None,
        )
        .ok()?;
    }
    Ok(OwnedKey(key))
}

fn open_key(
    path: &str,
    access: windows::Win32::System::Registry::REG_SAM_FLAGS,
) -> Result<OwnedKey> {
    let path = wide(path);
    let mut key = HKEY::default();
    unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(path.as_ptr()),
            None,
            access,
            &mut key,
        )
        .ok()?;
    }
    Ok(OwnedKey(key))
}

fn set_dword(key: &OwnedKey, name: &str, value: u32) -> Result<()> {
    let name = wide(name);
    unsafe {
        RegSetValueExW(
            key.0,
            PCWSTR(name.as_ptr()),
            None,
            REG_DWORD,
            Some(&value.to_le_bytes()),
        )
        .ok()
    }
}

fn query_dword(key: &OwnedKey, name: &str) -> Option<u32> {
    let name = wide(name);
    let mut kind = Default::default();
    let mut bytes = [0_u8; size_of::<u32>()];
    let mut length = bytes.len() as u32;
    let status = unsafe {
        RegQueryValueExW(
            key.0,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut kind),
            Some(bytes.as_mut_ptr()),
            Some(&mut length),
        )
    };
    (status.0 == 0 && kind == REG_DWORD && length == bytes.len() as u32)
        .then(|| u32::from_le_bytes(bytes))
}

fn set_string(key: &OwnedKey, name: Option<&str>, value: &str) -> Result<()> {
    let name = name.map(wide);
    let name = name
        .as_ref()
        .map_or_else(PCWSTR::null, |value| PCWSTR(value.as_ptr()));
    let value = wide(value);
    let bytes: Vec<_> = value
        .iter()
        .flat_map(|character| character.to_le_bytes())
        .collect();
    unsafe { RegSetValueExW(key.0, name, None, REG_SZ, Some(&bytes)).ok() }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn dock_to_dword(dock: ThumbnailDock) -> u32 {
    match dock {
        ThumbnailDock::Top => 0,
        ThumbnailDock::Bottom => 1,
        ThumbnailDock::Left => 2,
        ThumbnailDock::Right => 3,
    }
}

fn dword_to_dock(value: u32) -> ThumbnailDock {
    match value {
        0 => ThumbnailDock::Top,
        2 => ThumbnailDock::Left,
        3 => ThumbnailDock::Right,
        _ => ThumbnailDock::Bottom,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_menu_command_quotes_executable_and_image_path() {
        assert_eq!(
            context_menu_command(Path::new(r"C:\Program Files\PurePic\PurePic.exe")),
            r#""C:\Program Files\PurePic\PurePic.exe" "%1""#
        );
    }

    #[test]
    fn thumbnail_dock_values_round_trip() {
        for dock in [
            ThumbnailDock::Top,
            ThumbnailDock::Bottom,
            ThumbnailDock::Left,
            ThumbnailDock::Right,
        ] {
            assert_eq!(dword_to_dock(dock_to_dword(dock)), dock);
        }
        assert_eq!(dword_to_dock(99), ThumbnailDock::Bottom);
    }
}
