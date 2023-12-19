#[derive(Debug)]
pub enum ChademoError {
    DecodeBadId(u16),
    DecodeBadIdExt,
}
impl core::error::Error for ChademoError {}
impl core::fmt::Display for ChademoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ChademoError::*;
        match self {
            DecodeBadId(id) => write!(f, "Attemtped to decode invalid CAN ID {id}"),
            DecodeBadIdExt => write!(f, "Attemtped to decode invalid extended CAN ID"),
        }
    }
}
