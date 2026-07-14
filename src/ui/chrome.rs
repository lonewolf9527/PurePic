use crate::ui::layout::RectF;

pub const CAPTION_BUTTON_WIDTH_DIP: f32 = 46.0;
pub const TITLE_ACTION_BUTTON_WIDTH_DIP: f32 = 36.0;
pub const TITLE_ACTION_RIGHT_GAP_DIP: f32 = 12.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CaptionButton {
    #[default]
    None,
    Minimize,
    Maximize,
    Close,
}

pub fn title_action_button_rect(title_bar: RectF) -> RectF {
    let caption_left = title_bar.right() - CAPTION_BUTTON_WIDTH_DIP * 3.0;
    let right = caption_left - TITLE_ACTION_RIGHT_GAP_DIP;
    RectF::new(
        right - TITLE_ACTION_BUTTON_WIDTH_DIP,
        title_bar.y,
        TITLE_ACTION_BUTTON_WIDTH_DIP,
        title_bar.height,
    )
}

pub fn title_action_separator_x(title_bar: RectF) -> f32 {
    title_bar.right() - CAPTION_BUTTON_WIDTH_DIP * 3.0 - TITLE_ACTION_RIGHT_GAP_DIP * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_action_sits_left_of_caption_buttons() {
        let title = RectF::new(0.0, 0.0, 890.0, 44.0);
        let action = title_action_button_rect(title);
        assert_eq!(action, RectF::new(704.0, 0.0, 36.0, 44.0));
        assert_eq!(title_action_separator_x(title), 746.0);
    }
}
