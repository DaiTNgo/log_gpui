//! When a field that takes a pattern (the template of a Log Format) or a name shows its error: never while the user
//! is typing, only once they stop for [`SETTLE`] or leave the field, and no
//! longer as soon as they type again (rule 40). Whether the current text is
//! usable is known at every keystroke, so Save can be off before any error
//! shows.

use std::time::Duration;

/// How long the user stops typing before an error shows.
pub const SETTLE: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternField<E> {
    error: Option<E>,
    shown: bool,
    /// Bumped by every edit; a settle timer of an older edit does nothing.
    edits: u64,
}

impl<E> Default for PatternField<E> {
    fn default() -> Self {
        Self { error: None, shown: false, edits: 0 }
    }
}

impl<E> PatternField<E> {
    /// The text changed; `result` says whether it is usable. Hides the
    /// error. Returns the edit to pass to [`settle`](Self::settle) after
    /// [`SETTLE`].
    pub fn edit(&mut self, result: Result<(), E>) -> u64 {
        self.edits += 1;
        self.shown = false;
        self.error = result.err();
        self.edits
    }

    /// The user stopped typing after `edit`: show its error, unless they
    /// typed again since.
    pub fn settle(&mut self, edit: u64) -> bool {
        if edit == self.edits && self.error.is_some() && !self.shown {
            self.shown = true;
            return true;
        }
        false
    }

    /// The user left the field (or pressed Enter): show the error now.
    pub fn leave(&mut self) {
        self.shown = self.error.is_some();
    }

    pub fn is_valid(&self) -> bool {
        self.error.is_none()
    }

    /// The error to show, if it is time to show it.
    pub fn visible_error(&self) -> Option<&E> {
        self.error.as_ref().filter(|_| self.shown)
    }
}
