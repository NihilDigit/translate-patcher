pub const APP_NAME: &str = "translate-patcher";
pub const APP_DESCRIPTION: &str =
    "Embed external translation dictionaries into visual novel game resources.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    TyranoAsar,
}

impl Backend {
    pub fn label(self) -> &'static str {
        match self {
            Self::TyranoAsar => "TyranoScript / Electron ASAR",
        }
    }
}
