//! Log Editor Workspace Domain: the headless 3-tier state machine layer
//! (Workspace / Project -> Working Set -> Log Canvas) decoupled from GPUI
//! view components. This module is the primary test seam for the entire
//! workspace feature set as described in the log-editor-workspace PRD.

pub mod correlation;
pub mod investigation;
pub mod log_space;
pub mod new_space;
pub mod project;
pub mod scratch_store;
pub mod session;
pub mod state;
pub mod working_set;

pub use investigation::{CaseStatus, InvestigationCase, LogAnchor};
pub use log_space::{FileOrder, LogFile, LogSpace, LogSpaceKind};
pub use project::{Project, ProjectColorAccent};
pub use correlation::{CorrelationHit, CorrelationResult, nearest_timestamp_index, search_across_files};
pub use session::{SessionManifest, SessionPersistence};
pub use state::{ContextSidebarState, ContextSection, FocusPaneState, LeftSidebarState, WorkspaceState};
pub use working_set::{TabSessionState, TabTarget, ViewMode, WorkingSet, WorkspaceTab};