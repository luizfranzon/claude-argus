//! Close-confirmation orchestration for Sessions and Workspaces: the
//! request/confirm two-step from `close_requires_confirmation` (ADR-0006,
//! extended to Sessions by ADR-0010), routed to a `Modal` when confirmation
//! is required.

use argus_application::use_cases::CloseDecision;
use argus_domain::{SessionId, WorkspaceId};

use crate::i18n::t;

use super::{AppState, Modal};

impl AppState {
    pub fn request_close_session(&mut self, session_id: SessionId) {
        match self.runtime.request_close_session(session_id) {
            CloseDecision::RequiresConfirmation => {
                self.modal = Some(Modal::ConfirmCloseSession { session_id });
            }
            CloseDecision::AlreadyClosed => self.on_session_gone(session_id),
        }
    }

    pub fn confirm_close_session(&mut self, session_id: SessionId) {
        if let Err(e) = self.runtime.confirm_close_session(session_id) {
            self.set_status(t("session.close.error_status", &[("error", &e.to_string())]));
        }
        self.on_session_gone(session_id);
    }

    pub fn request_close_workspace(&mut self, workspace_id: WorkspaceId) {
        match self.runtime.request_close_workspace(workspace_id) {
            CloseDecision::RequiresConfirmation => {
                self.modal = Some(Modal::ConfirmCloseWorkspace { workspace_id });
            }
            CloseDecision::AlreadyClosed => self.remove_workspace(workspace_id),
        }
    }

    pub fn confirm_close_workspace(&mut self, workspace_id: WorkspaceId) {
        if let Err(e) = self.runtime.confirm_close_workspace(workspace_id) {
            self.set_status(t("workspace.close.error_status", &[("error", &e.to_string())]));
        }
        self.runtime.unwatch_workspace(workspace_id);
        self.remove_workspace(workspace_id);
    }

    fn remove_workspace(&mut self, workspace_id: WorkspaceId) {
        if let Some(entry) = self.workspace_entries.remove(&workspace_id) {
            for session_id in entry.sessions {
                self.sessions.remove(&session_id);
                self.stream_to_session.retain(|_, id| *id != session_id);
            }
        }
        self.workspaces.retain(|id| *id != workspace_id);
        if self.active_workspace == Some(workspace_id) {
            self.active_workspace = self.workspaces.first().copied();
            self.resize_focused_session();
        }
        if self.workspaces.is_empty() {
            self.should_quit = true;
        }
    }
}
