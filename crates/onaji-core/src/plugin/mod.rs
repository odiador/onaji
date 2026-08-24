pub mod manager;
pub mod manifest;
pub mod protocol;
pub mod subprocess;

pub use manager::PluginManager;
pub use manifest::{PluginManifest, PluginPermissions, PluginRuntime};
pub use protocol::*;
pub use subprocess::SubprocessPluginAdapter;
