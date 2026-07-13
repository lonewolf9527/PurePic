#![windows_subsystem = "windows"]

mod platform;

fn main() -> windows::core::Result<()> {
    platform::window::run()
}
