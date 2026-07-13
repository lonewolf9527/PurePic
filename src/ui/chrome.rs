#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CaptionButton {
    #[default]
    None,
    Minimize,
    Maximize,
    Close,
}
