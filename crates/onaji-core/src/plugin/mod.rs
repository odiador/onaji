pub mod installer;
pub mod lockfile;
pub mod manager;
pub mod manifest;
pub mod protocol;
pub mod registry_client;
pub mod subprocess;

pub use installer::PluginInstaller;
pub use lockfile::{LockedPlugin, LockfileError, OnajiLockfile};
pub use manager::PluginManager;
pub use manifest::{PluginManifest, PluginPermissions, PluginRuntime};
pub use protocol::{PluginInitializeParams, PluginInitializeResult};
pub use registry_client::{PluginPackageInfo, PluginRegistryClient, PluginRelease, RegistryError};
pub use subprocess::SubprocessPluginAdapter;
