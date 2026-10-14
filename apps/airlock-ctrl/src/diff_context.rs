//! Fold unchanged rows without altering source offsets, selections or the patch.
pub const CONTEXT_LINES: usize = 3;
const EXPANSION_LINES: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayRow {
    Source(usize),
    Gap { start: usize, end: usize },
}

pub struct ContextView {
    changed: Vec<bool>,
    visible: Vec<bool>,
    pub entries: Vec<DisplayRow>,
}

impl ContextView {
    pub fn new(changed: Vec<bool>) -> Self {
        let mut view = Self {
            visible: vec![false; changed.len()],
            changed,
            entries: Vec::new(),
        };
        view.reset();
        view
    }

    pub fn reset(&mut self) {
        self.visible.fill(false);
        for (index, changed) in self.changed.iter().enumerate() {
            if *changed {
                let start = index.saturating_sub(CONTEXT_LINES);
                let end = (index + CONTEXT_LINES + 1).min(self.visible.len());
                self.visible[start..end].fill(true);
            }
        }
        self.rebuild();
    }

    pub fn show_all(&mut self) {
        self.visible.fill(true);
        self.rebuild();
    }

    pub fn expand(&mut self, start: usize, end: usize, all: bool) {
        if start >= end || end > self.visible.len() {
            return;
        }
        if all {
            self.visible[start..end].fill(true);
        } else {
            // Reveal next to existing context. A middle gap opens at both ends;
            // a file's leading/trailing gap opens toward its nearby change.
            if start > 0 || end == self.visible.len() {
                self.visible[start..(start + EXPANSION_LINES).min(end)].fill(true);
            }
            if end < self.visible.len() || start == 0 {
                self.visible[end.saturating_sub(EXPANSION_LINES).max(start)..end].fill(true);
            }
        }
        self.rebuild();
    }

    fn rebuild(&mut self) {
        self.entries.clear();
        let mut index = 0;
        while index < self.visible.len() {
            if self.visible[index] {
                self.entries.push(DisplayRow::Source(index));
                index += 1;
            } else {
                let start = index;
                while index < self.visible.len() && !self.visible[index] {
                    index += 1;
                }
                self.entries.push(DisplayRow::Gap { start, end: index });
            }
        }
    }
}

/// Navigation follows changed source rows, including aligned replacement pairs.
/// Metadata-only files have no changed lines, but remain accessible in the tree.
pub fn adjacent_change(
    changes: &[(usize, usize)],
    current: Option<usize>,
    file: usize,
    forward: bool,
) -> Option<usize> {
    if let Some(index) = current {
        if forward {
            return index.checked_add(1).filter(|next| *next < changes.len());
        }
        return index.checked_sub(1);
    }
    if forward {
        changes.iter().position(|(index, _)| *index >= file)
    } else {
        changes.iter().rposition(|(index, _)| *index <= file)
    }
}
