use std::ops::Range;

use crate::midi::TimedEvent;

pub const MAX_EVENTS_PER_POLL: usize = 512;
pub const MAX_RECOVERY_NOTES: usize = 128;

#[derive(Debug, PartialEq)]
pub enum DispatchPlan {
    Events(Range<usize>),
    Resync { next_cursor: usize },
}

/// A suspended tab or extreme same-tick burst must not monopolize the UI
/// thread. Normal batches retain their exact event order. An oversized
/// backlog resumes at musical time instead of replaying every obsolete hit.
pub fn plan(events: &[TimedEvent], cursor: usize, target_tick: u64) -> DispatchPlan {
    let end = events
        .partition_point(|event| event.tick <= target_tick)
        .max(cursor);
    if end - cursor > MAX_EVENTS_PER_POLL {
        DispatchPlan::Resync { next_cursor: end }
    } else {
        DispatchPlan::Events(cursor..end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi::EventKind;

    fn events(count: usize, same_tick: bool) -> Vec<TimedEvent> {
        (0..count)
            .map(|i| TimedEvent {
                tick: if same_tick { 0 } else { i as u64 },
                track: 0,
                channel: 0,
                kind: EventKind::NoteOn {
                    note: 60,
                    velocity: 100,
                },
            })
            .collect()
    }

    #[test]
    fn ordinary_batches_keep_order_and_boundary_events() {
        let events = events(1_000, false);
        assert_eq!(plan(&events, 10, 20), DispatchPlan::Events(10..21));
        assert_eq!(plan(&events, 21, 20), DispatchPlan::Events(21..21));
        assert_eq!(plan(&events, 0, 511), DispatchPlan::Events(0..512));
        assert_eq!(
            plan(&events, 0, 512),
            DispatchPlan::Resync { next_cursor: 513 }
        );
    }

    #[test]
    fn stalled_tabs_and_huge_chords_do_not_replay_unbounded_batches() {
        assert_eq!(
            plan(&events(100_000, false), 0, 90_000),
            DispatchPlan::Resync {
                next_cursor: 90_001
            }
        );
        assert_eq!(
            plan(&events(100_000, true), 0, 0),
            DispatchPlan::Resync {
                next_cursor: 100_000
            }
        );
        assert_eq!(plan(&[], 0, u64::MAX), DispatchPlan::Events(0..0));
    }
}
