#![windows_subsystem = "windows"]

mod image;
mod platform;
mod render;

fn main() -> windows::core::Result<()> {
    let image_path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    platform::window::run(image_path)
}
