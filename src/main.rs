#![windows_subsystem = "windows"]

mod platform;
mod render;

fn main() -> windows::core::Result<()> {
    platform::window::run()
}
