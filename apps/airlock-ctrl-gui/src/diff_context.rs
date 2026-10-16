//! Fold unchanged rows without altering source offsets, selections or the patch.
pub const CONTEXT_LINES: usize = 3;
const EXPANSION_LINES: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayRow {
    Source(usize),
    Gap { start: usize, end: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Section {
    pub start: usize,
    pub end: usize,
    pub first_change: usize,
}

/// A section is one hunk at the default context size. Nearby edits belong to
/// the same section when their context touches. Expanding gaps does not change
/// these navigation destinations.
fn sections(changed: &[bool]) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    for (index, changed_row) in changed.iter().enumerate() {
        if !changed_row {
            continue;
        }
        let start = index.saturating_sub(CONTEXT_LINES);
        let end = (index + CONTEXT_LINES + 1).min(changed.len());
        if let Some(previous) = sections.last_mut()
            && previous.end >= start
        {
            previous.end = end;
        } else {
            sections.push(Section {
                start,
                end,
                first_change: index,
            });
        }
    }
    sections
}

pub struct ContextView {
    pub sections: Vec<Section>,
    visible: Vec<bool>,
    pub entries: Vec<DisplayRow>,
}

impl ContextView {
    pub fn new(changed: Vec<bool>) -> Self {
        let mut view = Self {
            visible: vec![false; changed.len()],
            sections: sections(&changed),
            entries: Vec::new(),
        };
        view.reset();
        view
    }

    pub fn reset(&mut self) {
        self.visible.fill(false);
        for section in &self.sections {
            self.visible[section.start..section.end].fill(true);
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

/// Metadata-only files have no diff sections, but remain accessible in the tree.
pub fn adjacent_section(
    sections: &[(usize, Section)],
    current: Option<usize>,
    file: usize,
    forward: bool,
) -> Option<usize> {
    if let Some(index) = current {
        if forward {
            return index.checked_add(1).filter(|next| *next < sections.len());
        }
        return index.checked_sub(1);
    }
    if forward {
        sections.iter().position(|(index, _)| *index >= file)
    } else {
        sections.iter().rposition(|(index, _)| *index <= file)
    }
}
