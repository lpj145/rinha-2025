use std::fmt::Display;

pub mod http;
pub mod socket;

pub struct CorrelationId(pub [u8; 36]);

unsafe impl Send for CorrelationId {}
unsafe impl Sync for CorrelationId {}

impl Display for CorrelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(&self.0))
    }
}
