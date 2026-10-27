//! What local changes replaced, so they can be undone: per step (one user action), the notes and
//! folders as they were before it. Text edits are not here: the editor undoes those.

/// Steps kept; older ones are forgotten.
const KEEP: usize = 100;

/// Something as it was before a step. New notes and folders are not here: undoing one would
/// take what was typed into it since.
#[derive(Debug, Clone)]
pub(crate) enum Before {
    /// Note `id` (ids are never reused) was at `path`; `text`: it was deleted, with that text.
    Note {
        id: i64,
        path: String,
        text: Option<String>,
    },
    /// Folder `path` was deleted.
    Folder(String),
    /// A folder was at `from`, now at `to`.
    FolderMoved { from: String, to: String },
}

#[derive(Default)]
pub(crate) struct Journal {
    steps: Vec<(u64, Vec<Before>)>,
    next: u64,
}

impl Journal {
    pub fn record(&mut self, before: Vec<Before>) {
        if before.is_empty() {
            return;
        }
        self.next += 1;
        self.steps.push((self.next, before));
        if self.steps.len() > KEEP {
            self.steps.remove(0);
        }
    }

    pub fn last(&self) -> Option<u64> {
        self.steps.last().map(|s| s.0)
    }

    /// Remove step `id` (else the latest) and return what it replaced.
    pub fn take(&mut self, id: Option<u64>) -> Option<Vec<Before>> {
        let i = match id {
            Some(id) => self.steps.iter().position(|s| s.0 == id)?,
            None => self.steps.len().checked_sub(1)?,
        };
        Some(self.steps.remove(i).1)
    }

    pub fn clear(&mut self) {
        self.steps.clear();
    }
}
