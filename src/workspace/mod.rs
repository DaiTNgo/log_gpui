//! Log Editor Workspace Domain: the headless 3-tier state machine layer
//! (Workspace / Project -> Working Set -> Log Canvas) decoupled from GPUI
//! view components. This module is the primary test seam for the entire
//! workspace feature set as described in the log-editor-workspace PRD.

pub mod investigation;
pub mod project;
pub mod saved_view;
pub mod session;
pub mod state;
pub mod working_set;

pub use investigation::{CaseStatus, InvestigationCase, LogAnchor};
pub use project::{LogFileDescriptor, Project, ProjectColorAccent};
pub use saved_view::{CorrelationHit, CorrelationResult, SavedView, nearest_timestamp_index, search_across_files};
pub use session::{SessionManifest, SessionPersistence};
pub use state::{ContextSidebarState, ContextTab, LeftSidebarState, WorkspaceState};
pub use working_set::{TabSessionState, TabTarget, WorkingSet, WorkspaceTab};