use std::env;
use std::path::{Path, PathBuf};

use purepic::ui::icon::Icon;

pub struct IconSet {
    pub window_minimize: Icon,
    pub window_maximize: Icon,
    pub window_restore: Icon,
    pub window_close: Icon,
    pub actual_size: Icon,
    pub chevron_down: Icon,
    pub zoom_out: Icon,
    pub zoom_in: Icon,
    pub fullscreen: Icon,
}

impl IconSet {
    pub fn load() -> Self {
        let directory = find_icon_directory();
        Self {
            window_minimize: load(&directory, "window-minimize.svg"),
            window_maximize: load(&directory, "window-maximize.svg"),
            window_restore: load(&directory, "window-restore.svg"),
            window_close: load(&directory, "window-close.svg"),
            actual_size: load(&directory, "actual-size.svg"),
            chevron_down: load(&directory, "chevron-down.svg"),
            zoom_out: load(&directory, "zoom-out.svg"),
            zoom_in: load(&directory, "zoom-in.svg"),
            fullscreen: load(&directory, "fullscreen.svg"),
        }
    }
}

fn load(directory: &Path, name: &str) -> Icon {
    Icon::load(&directory.join(name)).unwrap_or_default()
}

fn find_icon_directory() -> PathBuf {
    if let Ok(executable) = env::current_exe()
        && let Some(directory) = executable.parent()
    {
        let candidate = directory.join("Assets").join("icons");
        if candidate.is_dir() {
            return candidate;
        }
    }
    if let Ok(current) = env::current_dir() {
        let candidate = current.join("Assets").join("icons");
        if candidate.is_dir() {
            return candidate;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("Assets")
        .join("icons")
}
