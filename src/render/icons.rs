use purepic::ui::icon::Icon;

pub struct IconSet {
    pub app: Icon,
    pub context_register: Icon,
    pub context_unregister: Icon,
    pub thumbnails: Icon,
    pub dock_top: Icon,
    pub dock_bottom: Icon,
    pub dock_left: Icon,
    pub dock_right: Icon,
    pub image_previous: Icon,
    pub image_next: Icon,
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
            thumbnails: load(include_str!("../../Assets/icons/thumbnails.svg")),
            dock_top: load(include_str!("../../Assets/icons/dock-top.svg")),
            dock_bottom: load(include_str!("../../Assets/icons/dock-bottom.svg")),
            dock_left: load(include_str!("../../Assets/icons/dock-left.svg")),
            dock_right: load(include_str!("../../Assets/icons/dock-right.svg")),
            image_previous: load(include_str!("../../Assets/icons/image-previous.svg")),
            image_next: load(include_str!("../../Assets/icons/image-next.svg")),
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
        for icon in [&icons.context_register, &icons.context_unregister] {
            assert_eq!((icon.width, icon.height), (24.0, 24.0));
            assert!(icon.paths.iter().all(|path| !path.fill && path.stroke));
            assert!(
                icon.paths
                    .iter()
                    .all(|path| (path.stroke_width - 0.8).abs() < f32::EPSILON)
            );
        }
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
