pub const MIN_ZOOM: f64 = 0.01;
pub const MAX_ZOOM: f64 = 8.0;

pub const ZOOM_STEPS: &[f64] = &[
    0.01, 0.02, 0.05, 0.10, 0.25, 0.50, 0.75, 1.00, 1.25, 1.50, 2.00, 3.00, 4.00, 5.00, 6.00, 7.00,
    8.00,
];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointF {
    pub x: f64,
    pub y: f64,
}

impl PointF {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SizeF {
    pub width: f64,
    pub height: f64,
}

impl SizeF {
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

pub fn fit_zoom(image: SizeF, viewport: SizeF) -> f64 {
    if image.width <= 0.0 || image.height <= 0.0 || viewport.width <= 0.0 || viewport.height <= 0.0
    {
        return 1.0;
    }

    (viewport.width / image.width)
        .min(viewport.height / image.height)
        .clamp(MIN_ZOOM, MAX_ZOOM)
}

pub fn slider_to_zoom(position: f64) -> f64 {
    let position = position.clamp(0.0, 1.0);
    let log_min = MIN_ZOOM.ln();
    let log_max = MAX_ZOOM.ln();
    (log_min + position * (log_max - log_min)).exp()
}

pub fn zoom_to_slider(zoom: f64) -> f64 {
    let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    let log_min = MIN_ZOOM.ln();
    let log_max = MAX_ZOOM.ln();
    ((zoom.ln() - log_min) / (log_max - log_min)).clamp(0.0, 1.0)
}

pub fn step_zoom(current: f64, direction: i32) -> f64 {
    let current = current.clamp(MIN_ZOOM, MAX_ZOOM);
    const EPSILON: f64 = 1e-9;

    if direction > 0 {
        ZOOM_STEPS
            .iter()
            .copied()
            .find(|step| *step > current + EPSILON)
            .unwrap_or(MAX_ZOOM)
    } else if direction < 0 {
        ZOOM_STEPS
            .iter()
            .rev()
            .copied()
            .find(|step| *step < current - EPSILON)
            .unwrap_or(MIN_ZOOM)
    } else {
        current
    }
}

pub fn origin_after_zoom(
    old_origin: PointF,
    anchor_in_viewport: PointF,
    old_zoom: f64,
    new_zoom: f64,
) -> PointF {
    let old_zoom = old_zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    let new_zoom = new_zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    let image_x = (anchor_in_viewport.x - old_origin.x) / old_zoom;
    let image_y = (anchor_in_viewport.y - old_origin.y) / old_zoom;

    PointF::new(
        anchor_in_viewport.x - image_x * new_zoom,
        anchor_in_viewport.y - image_y * new_zoom,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-9;

    #[test]
    fn fit_zoom_preserves_the_entire_image() {
        let zoom = fit_zoom(SizeF::new(3840.0, 2160.0), SizeF::new(1280.0, 700.0));
        assert!((zoom - 700.0 / 2160.0).abs() < EPSILON);
    }

    #[test]
    fn slider_mapping_round_trips() {
        for expected in [MIN_ZOOM, 0.10, 0.56, 1.0, 8.0, MAX_ZOOM] {
            let position = zoom_to_slider(expected);
            let actual = slider_to_zoom(position);
            assert!((actual - expected).abs() < EPSILON);
        }
    }

    #[test]
    fn zoom_steps_follow_presets() {
        assert_eq!(step_zoom(0.56, 1), 0.75);
        assert_eq!(step_zoom(0.56, -1), 0.50);
        assert_eq!(step_zoom(1.0, 1), 1.25);
        assert_eq!(step_zoom(1.0, -1), 0.75);
        assert_eq!(step_zoom(MAX_ZOOM, 1), MAX_ZOOM);
        assert_eq!(step_zoom(MAX_ZOOM, -1), 7.0);
    }

    #[test]
    fn anchored_zoom_keeps_the_same_image_point_under_cursor() {
        let old_origin = PointF::new(20.0, 30.0);
        let anchor = PointF::new(220.0, 130.0);
        let new_origin = origin_after_zoom(old_origin, anchor, 1.0, 2.0);

        assert_eq!(new_origin, PointF::new(-180.0, -70.0));
        let old_image_point = PointF::new(
            (anchor.x - old_origin.x) / 1.0,
            (anchor.y - old_origin.y) / 1.0,
        );
        let new_image_point = PointF::new(
            (anchor.x - new_origin.x) / 2.0,
            (anchor.y - new_origin.y) / 2.0,
        );
        assert_eq!(old_image_point, new_image_point);
    }
}
