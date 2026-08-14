/// A lifecycle status that has a terminating state. Implemented by both
/// `SessionStatus` and `WorkspaceStatus` so the close-confirmation policy in
/// [`requires_confirmation`] is expressed once instead of once per status enum.
pub trait Terminatable {
    fn is_terminating(&self) -> bool;
}

/// Whether closing something in this lifecycle status requires user
/// confirmation: true for every status except the terminating one. A future
/// "only confirm if busy" rule touches this one function for both Session
/// and Workspace, rather than two near-identical copies.
pub fn requires_confirmation<S: Terminatable>(status: S) -> bool {
    !status.is_terminating()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    enum Fake {
        Live,
        Done,
    }

    impl Terminatable for Fake {
        fn is_terminating(&self) -> bool {
            matches!(self, Fake::Done)
        }
    }

    #[test]
    fn live_status_requires_confirmation() {
        assert!(requires_confirmation(Fake::Live));
    }

    #[test]
    fn terminating_status_does_not_require_confirmation() {
        assert!(!requires_confirmation(Fake::Done));
    }
}
