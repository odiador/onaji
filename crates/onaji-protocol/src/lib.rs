pub mod capabilities;
pub mod error;
pub mod messages;

pub use capabilities::{ClientCapabilities, ServerCapabilities};
pub use error::{ErrorCode, RpcError};
pub use messages::*;

/// Protocol version identifier
pub const PROTOCOL_VERSION: &str = "0.1.0";
