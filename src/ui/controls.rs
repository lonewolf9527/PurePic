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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StatusControlsLayout {
    pub actual_size: RectF,
    pub zoom_menu: RectF,
    pub zoom_out: RectF,
    pub slider: RectF,
    pub zoom_in: RectF,
    pub fullscreen: RectF,
}

impl StatusControlsLayout {
    pub fn compute(status_bar: RectF) -> Self {
        const BUTTON: f32 = 36.0;
        const MENU: f32 = 78.0;
        const SLIDER: f32 = 120.0;
        const GAP: f32 = 4.0;
        const RIGHT_PADDING: f32 = 12.0;
        let total = BUTTON * 4.0 + MENU + SLIDER + GAP * 5.0;
        let y = status_bar.y + (status_bar.height - BUTTON).max(0.0) * 0.5;
        let mut x = (status_bar.right() - RIGHT_PADDING - total).max(status_bar.x);

        let actual_size = RectF::new(x, y, BUTTON, BUTTON);
        x += BUTTON + GAP;
        let zoom_menu = RectF::new(x, y, MENU, BUTTON);
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
}
