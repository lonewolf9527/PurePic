use purepic::ui::icon::Icon;

pub struct IconSet {
    pub app: Icon,
    pub context_register: Icon,
    pub context_unregister: Icon,
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
        Self {
            app: load(include_str!("../../Assets/icons/app.svg")),
            context_register: load(include_str!("../../Assets/icons/context-register.svg")),
            context_unregister: load(include_str!("../../Assets/icons/context-unregister.svg")),
            window_minimize: load(include_str!("../../Assets/icons/window-minimize.svg")),
            window_maximize: load(include_str!("../../Assets/icons/window-maximize.svg")),
            window_restore: load(include_str!("../../Assets/icons/window-restore.svg")),
            window_close: load(include_str!("../../Assets/icons/window-close.svg")),
            actual_size: load(include_str!("../../Assets/icons/actual-size.svg")),
            chevron_down: load(include_str!("../../Assets/icons/chevron-down.svg")),
            zoom_out: load(include_str!("../../Assets/icons/zoom-out.svg")),
            zoom_in: load(include_str!("../../Assets/icons/zoom-in.svg")),
            fullscreen: load(include_str!("../../Assets/icons/fullscreen.svg")),
        }
    }
}

fn load(source: &str) -> Icon {
    Icon::from_svg(source).expect("embedded SVG icon must be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_icons_are_valid_and_caption_strokes_are_thin() {
        let icons = IconSet::load();
        assert!(!icons.app.paths.is_empty());
        for icon in [
            icons.window_minimize,
            icons.window_maximize,
            icons.window_restore,
            icons.window_close,
        ] {
            assert!(
                icon.paths
                    .iter()
                    .all(|path| (path.stroke_width - 1.2).abs() < f32::EPSILON)
            );
        }
    }
}
