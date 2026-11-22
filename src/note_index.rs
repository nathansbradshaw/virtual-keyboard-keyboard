use crate::{midi::Note, work_budget::WorkBudget};

const BLOCK: usize = 32;

/// Maximum end tick per block, arranged as an interval tree. A long sustain
/// only keeps its own branch alive, rather than forcing a scan of all history.
#[derive(Debug, Clone)]
pub struct NoteIndex {
    leaves: usize,
    max_end: Vec<u64>,
}

impl NoteIndex {
    #[cfg(test)]
    pub fn new(notes: &[Note]) -> Self {
        crate::work_budget::complete_immediately(Self::build(notes, &mut WorkBudget::blocking()))
            .unwrap()
    }

    pub async fn build(notes: &[Note], budget: &mut WorkBudget) -> Result<Self, String> {
        let leaves = notes.len().div_ceil(BLOCK).max(1).next_power_of_two();
        let mut max_end = vec![0; leaves * 2];
        for (i, chunk) in notes.chunks(BLOCK).enumerate() {
            max_end[leaves + i] = chunk.iter().map(|note| note.end_tick).max().unwrap_or(0);
            budget.checkpoint().await?;
        }
        for i in (1..leaves).rev() {
            max_end[i] = max_end[i * 2].max(max_end[i * 2 + 1]);
            budget.checkpoint().await?;
        }
        Ok(Self { leaves, max_end })
    }

    pub fn query<'a>(&'a self, notes: &'a [Note], start: u64, end: u64) -> NoteWindow<'a> {
        let last = if start <= end {
            notes.partition_point(|note| note.start_tick <= end)
        } else {
            0
        };
        let mut pending = Vec::with_capacity(32);
        pending.push((1, 0, self.leaves));
        NoteWindow {
            index: self,
            notes,
            start,
            last,
            pending,
            current: [].iter(),
            #[cfg(test)]
            visited_nodes: 0,
        }
    }
}

pub struct NoteWindow<'a> {
    index: &'a NoteIndex,
    notes: &'a [Note],
    start: u64,
    last: usize,
    pending: Vec<(usize, usize, usize)>,
    current: std::slice::Iter<'a, Note>,
    #[cfg(test)]
    visited_nodes: usize,
}

impl<'a> Iterator for NoteWindow<'a> {
    type Item = &'a Note;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(note) = self.current.find(|note| note.end_tick >= self.start) {
                return Some(note);
            }
            let (node, left, right) = self.pending.pop()?;
            #[cfg(test)]
            {
                self.visited_nodes += 1;
            }
            if left * BLOCK >= self.last || self.index.max_end[node] < self.start {
                continue;
            }
            if right - left == 1 {
                self.current = self.notes[left * BLOCK..((left + 1) * BLOCK).min(self.last)].iter();
            } else {
                let middle = (left + right) / 2;
                self.pending.push((node * 2 + 1, middle, right));
                self.pending.push((node * 2, left, middle));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn notes(count: usize) -> Vec<Note> {
        (0..count)
            .map(|i| Note {
                start_tick: i as u64 * 10,
                end_tick: if i == 0 { u64::MAX } else { i as u64 * 10 + 8 },
                midi_note: 60,
                channel: 0,
                track: 0,
                velocity: 100,
            })
            .collect()
    }
    #[test]
    fn long_sustain_does_not_scan_expired_history() {
        let notes = notes(1_000_000);
        let index = NoteIndex::new(&notes);
        let mut window = index.query(&notes, 9_000_001, 9_000_005);
        assert_eq!(
            window.by_ref().map(|n| n.start_tick).collect::<Vec<_>>(),
            [0, 9_000_000]
        );
        assert!(
            window.visited_nodes < 100,
            "visited {} tree nodes",
            window.visited_nodes
        );
    }
    #[test]
    fn queries_preserve_order_sustains_and_inclusive_boundaries() {
        let notes = notes(1000);
        let index = NoteIndex::new(&notes);
        for (a, b) in [(0, 0), (8, 10), (9, 9), (315, 999), (10001, 20000)] {
            let expected: Vec<_> = notes
                .iter()
                .filter(|n| n.start_tick <= b && n.end_tick >= a)
                .map(|n| n.start_tick)
                .collect();
            assert_eq!(
                index
                    .query(&notes, a, b)
                    .map(|n| n.start_tick)
                    .collect::<Vec<_>>(),
                expected
            );
        }
        assert_eq!(index.query(&notes, 10, 0).count(), 0);
        assert_eq!(NoteIndex::new(&[]).query(&[], 0, 0).count(), 0);
    }
}
