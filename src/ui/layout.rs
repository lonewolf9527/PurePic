pub const DEFAULT_DPI: u32 = 96;
pub const TITLE_BAR_HEIGHT_DIP: f32 = 36.0;
pub const STATUS_BAR_HEIGHT_DIP: f32 = 44.0;
pub const HORIZONTAL_THUMBNAIL_EXTENT_DIP: f32 = 112.0;
pub const VERTICAL_THUMBNAIL_EXTENT_DIP: f32 = 120.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RectF {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl RectF {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ThumbnailDock {
    Top,
    #[default]
    Bottom,
    Left,
    Right,
}

impl ThumbnailDock {
    pub fn default_extent_dip(self) -> f32 {
        match self {
            Self::Top | Self::Bottom => HORIZONTAL_THUMBNAIL_EXTENT_DIP,
            Self::Left | Self::Right => VERTICAL_THUMBNAIL_EXTENT_DIP,
        }
    }

    pub fn is_horizontal(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LayoutInput {
    pub client_width_px: u32,
    pub client_height_px: u32,
    pub dpi: u32,
    pub thumbnail_visible: bool,
    pub thumbnail_dock: ThumbnailDock,
    pub thumbnail_extent_dip: f32,
}

impl LayoutInput {
    pub fn new(client_width_px: u32, client_height_px: u32, dpi: u32) -> Self {
        Self {
            client_width_px,
            client_height_px,
            dpi,
            thumbnail_visible: false,
            thumbnail_dock: ThumbnailDock::Bottom,
            thumbnail_extent_dip: HORIZONTAL_THUMBNAIL_EXTENT_DIP,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WindowLayout {
    pub client: RectF,
    pub title_bar: RectF,
    pub canvas: RectF,
    pub thumbnail_panel: Option<RectF>,
    pub status_bar: RectF,
}

pub fn compute_layout(input: LayoutInput) -> WindowLayout {
    let dpi = input.dpi.max(1) as f32;
    let width = input.client_width_px as f32 * DEFAULT_DPI as f32 / dpi;
    let height = input.client_height_px as f32 * DEFAULT_DPI as f32 / dpi;

    let title_height = TITLE_BAR_HEIGHT_DIP.min(height);
    let remaining_after_title = (height - title_height).max(0.0);
    let status_height = STATUS_BAR_HEIGHT_DIP.min(remaining_after_title);
    let content_height = (remaining_after_title - status_height).max(0.0);
    let content = RectF::new(0.0, title_height, width, content_height);

    let mut canvas = content;
    let mut thumbnail_panel = None;

    if input.thumbnail_visible && width > 0.0 && content_height > 0.0 {
        let requested_extent = input.thumbnail_extent_dip.max(0.0);

        match input.thumbnail_dock {
            ThumbnailDock::Top => {
                let extent = requested_extent.min(content.height);
                thumbnail_panel = Some(RectF::new(content.x, content.y, content.width, extent));
                canvas.y += extent;
                canvas.height -= extent;
            }
            ThumbnailDock::Bottom => {
                let extent = requested_extent.min(content.height);
                canvas.height -= extent;
                thumbnail_panel = Some(RectF::new(
                    content.x,
                    canvas.bottom(),
                    content.width,
                    extent,
                ));
            }
            ThumbnailDock::Left => {
                let extent = requested_extent.min(content.width);
                thumbnail_panel = Some(RectF::new(content.x, content.y, extent, content.height));
                canvas.x += extent;
                canvas.width -= extent;
            }
            ThumbnailDock::Right => {
                let extent = requested_extent.min(content.width);
                canvas.width -= extent;
                thumbnail_panel = Some(RectF::new(
                    canvas.right(),
                    content.y,
                    extent,
                    content.height,
                ));
            }
        }
    }

    WindowLayout {
        client: RectF::new(0.0, 0.0, width, height),
        title_bar: RectF::new(0.0, 0.0, width, title_height),
        canvas,
        thumbnail_panel,
        status_bar: RectF::new(0.0, title_height + content_height, width, status_height),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 0.001;

    fn assert_rect(actual: RectF, expected: RectF) {
        assert!((actual.x - expected.x).abs() < EPSILON);
        assert!((actual.y - expected.y).abs() < EPSILON);
        assert!((actual.width - expected.width).abs() < EPSILON);
        assert!((actual.height - expected.height).abs() < EPSILON);
    }

    #[test]
    fn hidden_thumbnail_uses_all_content_space() {
        let layout = compute_layout(LayoutInput::new(1280, 800, 96));

        assert_rect(layout.title_bar, RectF::new(0.0, 0.0, 1280.0, 36.0));
        assert_rect(layout.canvas, RectF::new(0.0, 36.0, 1280.0, 720.0));
        assert_rect(layout.status_bar, RectF::new(0.0, 756.0, 1280.0, 44.0));
        assert_eq!(layout.thumbnail_panel, None);
    }

    #[test]
    fn dpi_changes_physical_pixels_but_not_dip_layout() {
        let normal = compute_layout(LayoutInput::new(1280, 800, 96));
        let high_dpi = compute_layout(LayoutInput::new(2560, 1600, 192));

        assert_eq!(normal, high_dpi);
    }

    #[test]
    fn docks_thumbnail_panel_on_every_edge() {
        let expected = [
            (
                ThumbnailDock::Top,
                RectF::new(0.0, 36.0, 1280.0, 112.0),
                RectF::new(0.0, 148.0, 1280.0, 608.0),
            ),
            (
                ThumbnailDock::Bottom,
                RectF::new(0.0, 644.0, 1280.0, 112.0),
                RectF::new(0.0, 36.0, 1280.0, 608.0),
            ),
            (
                ThumbnailDock::Left,
                RectF::new(0.0, 36.0, 120.0, 720.0),
                RectF::new(120.0, 36.0, 1160.0, 720.0),
            ),
            (
                ThumbnailDock::Right,
                RectF::new(1160.0, 36.0, 120.0, 720.0),
                RectF::new(0.0, 36.0, 1160.0, 720.0),
            ),
        ];

        for (dock, expected_panel, expected_canvas) in expected {
            let input = LayoutInput {
                thumbnail_visible: true,
                thumbnail_dock: dock,
                thumbnail_extent_dip: dock.default_extent_dip(),
                ..LayoutInput::new(1280, 800, 96)
            };
            let layout = compute_layout(input);

            assert_rect(layout.thumbnail_panel.unwrap(), expected_panel);
            assert_rect(layout.canvas, expected_canvas);
        }
    }

    #[test]
    fn tiny_window_never_produces_negative_rectangles() {
        let input = LayoutInput {
            thumbnail_visible: true,
            thumbnail_extent_dip: 500.0,
            ..LayoutInput::new(20, 20, 96)
        };
        let layout = compute_layout(input);

        for rect in [
            layout.client,
            layout.title_bar,
            layout.canvas,
            layout.status_bar,
            layout.thumbnail_panel.unwrap_or_default(),
        ] {
            assert!(rect.width >= 0.0);
            assert!(rect.height >= 0.0);
        }
    }
}
