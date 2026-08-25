pub mod definition;
pub mod graph;

pub use definition::{CrossWorkspaceDep, Workspace, WorkspaceId};
pub use graph::{CrossWorkspaceImpact, MultiWorkspaceGraph, MultiWorkspaceImpactReport};
