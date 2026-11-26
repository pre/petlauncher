use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Tells a running search that its query is old. The clones of a token share
/// one state.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clone_sees_the_cancel_of_the_original() {
        let token = CancelToken::new();
        let clone = token.clone();

        token.cancel();

        assert!(clone.is_cancelled());
    }

    #[test]
    fn a_new_token_is_not_cancelled() {
        assert!(!CancelToken::new().is_cancelled());
    }
}
