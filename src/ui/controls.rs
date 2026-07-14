use crate::ui::layout::RectF;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusControl {
    ActualSize,
    ZoomMenu,
    ZoomOut,
    Slider,
    ZoomIn,
    Fullscreen,
}

impl StatusControl {
    pub const fn tooltip(self) -> Option<(&'static str, f32)> {
        match self {
            Self::ActualSize => Some(("实际大小", 76.0)),
            Self::ZoomMenu => Some(("选择缩放比例", 104.0)),
            Self::ZoomOut => Some(("缩小", 52.0)),
            Self::Slider => None,
            Self::ZoomIn => Some(("放大", 52.0)),
            Self::Fullscreen => Some(("全屏 (F11)", 84.0)),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StatusControlsLayout {
    pub actual_size: RectF,
    pub zoom_menu: RectF,
    pub zoom_out: RectF,
    pub slider: RectF,
    pub zoom_in: RectF,
    pub fullscreen: RectF,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThumbnailControl {
    Toggle,
    DockMenu,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ThumbnailControlsLayout {
    pub toggle: RectF,
    pub dock_menu: RectF,
}

impl ThumbnailControlsLayout {
    pub fn compute(status_bar: RectF) -> Self {
        const BUTTON: f32 = 36.0;
        const DOCK_MENU: f32 = 48.0;
        let y = status_bar.y + (status_bar.height - BUTTON).max(0.0) * 0.5;
        Self {
            toggle: RectF::new(status_bar.x + 12.0, y, BUTTON, BUTTON),
            dock_menu: RectF::new(status_bar.x + 52.0, y, DOCK_MENU, BUTTON),
        }
    }

    pub fn hit_test(self, x: f32, y: f32) -> Option<ThumbnailControl> {
        if self.toggle.contains(x, y) {
            Some(ThumbnailControl::Toggle)
        } else if self.dock_menu.contains(x, y) {
            Some(ThumbnailControl::DockMenu)
        } else {
            None
        }
    }
}

impl StatusControlsLayout {
    pub fn compute(status_bar: RectF) -> Self {
        const BUTTON: f32 = 36.0;
        const MENU_HEIGHT: f32 = 32.0;
        const MENU: f32 = 78.0;
        const SLIDER: f32 = 120.0;
        const GAP: f32 = 4.0;
        const RIGHT_PADDING: f32 = 12.0;
        let total = BUTTON * 4.0 + MENU + SLIDER + GAP * 5.0;
        let y = status_bar.y + (status_bar.height - BUTTON).max(0.0) * 0.5;
        let menu_y = status_bar.y + (status_bar.height - MENU_HEIGHT).max(0.0) * 0.5;
        let mut x = (status_bar.right() - RIGHT_PADDING - total).max(status_bar.x);

        let actual_size = RectF::new(x, y, BUTTON, BUTTON);
        x += BUTTON + GAP;
        let zoom_menu = RectF::new(x, menu_y, MENU, MENU_HEIGHT);
        x += MENU + GAP;
        let zoom_out = RectF::new(x, y, BUTTON, BUTTON);
        x += BUTTON + GAP;
        let slider = RectF::new(x, y, SLIDER, BUTTON);
        x += SLIDER + GAP;
        let zoom_in = RectF::new(x, y, BUTTON, BUTTON);
        x += BUTTON + GAP;
        let fullscreen = RectF::new(x, y, BUTTON, BUTTON);

        Self {
            actual_size,
            zoom_menu,
            zoom_out,
            slider,
            zoom_in,
            fullscreen,
        }
    }

    pub fn hit_test(self, x: f32, y: f32) -> Option<StatusControl> {
        [
            (StatusControl::ActualSize, self.actual_size),
            (StatusControl::ZoomMenu, self.zoom_menu),
            (StatusControl::ZoomOut, self.zoom_out),
            (StatusControl::Slider, self.slider),
            (StatusControl::ZoomIn, self.zoom_in),
            (StatusControl::Fullscreen, self.fullscreen),
        ]
        .into_iter()
        .find_map(|(control, rect)| rect.contains(x, y).then_some(control))
    }

    pub fn rect(self, control: StatusControl) -> RectF {
        match control {
            StatusControl::ActualSize => self.actual_size,
            StatusControl::ZoomMenu => self.zoom_menu,
            StatusControl::ZoomOut => self.zoom_out,
            StatusControl::Slider => self.slider,
            StatusControl::ZoomIn => self.zoom_in,
            StatusControl::Fullscreen => self.fullscreen,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_end_at_the_right_padding() {
        let bar = RectF::new(0.0, 700.0, 1280.0, 52.0);
        let layout = StatusControlsLayout::compute(bar);
        assert_eq!(layout.fullscreen.right(), 1268.0);
        assert_eq!(
            layout.hit_test(1250.0, 726.0),
            Some(StatusControl::Fullscreen)
        );
    }

    #[test]
    fn buttons_expose_tooltips_but_slider_does_not() {
        assert_eq!(
            StatusControl::ActualSize.tooltip(),
            Some(("实际大小", 76.0))
        );
        assert_eq!(StatusControl::Slider.tooltip(), None);
    }

    #[test]
    fn zoom_menu_is_shorter_and_remains_vertically_centered() {
        let bar = RectF::new(0.0, 700.0, 1280.0, 44.0);
        let layout = StatusControlsLayout::compute(bar);
        assert_eq!(layout.zoom_menu.height, 32.0);
        assert_eq!(layout.zoom_menu.y, 706.0);
        assert_eq!(layout.actual_size.height, 36.0);
        assert_eq!(layout.actual_size.y, 704.0);
    }

    #[test]
    fn thumbnail_controls_are_pinned_to_the_left() {
        let bar = RectF::new(0.0, 700.0, 1280.0, 44.0);
        let layout = ThumbnailControlsLayout::compute(bar);
        assert_eq!(layout.toggle, RectF::new(12.0, 704.0, 36.0, 36.0));
        assert_eq!(layout.dock_menu, RectF::new(52.0, 704.0, 48.0, 36.0));
        assert_eq!(
            layout.hit_test(70.0, 720.0),
            Some(ThumbnailControl::DockMenu)
        );
    }
}
