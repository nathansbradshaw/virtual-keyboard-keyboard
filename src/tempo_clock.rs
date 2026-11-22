use crate::midi::TempoChange;

#[derive(Debug, Clone)]
struct Segment {
    tick: u64,
    micros: u64,
    micros_per_beat: u32,
}

/// Pre-integrated tempo changes. Timing queries use two binary searches at
/// most, regardless of the song length or number of tempo changes.
#[derive(Debug, Clone)]
pub struct TempoClock {
    ticks_per_beat: u16,
    segments: Vec<Segment>,
}

fn narrow(value: u128) -> u64 {
    value.min(u64::MAX as u128) as u64
}

impl TempoClock {
    pub fn new(changes: &[TempoChange], ticks_per_beat: u16) -> Self {
        assert!(ticks_per_beat > 0);
        let mut segments = vec![Segment {
            tick: 0,
            micros: 0,
            micros_per_beat: 500_000,
        }];
        for change in changes {
            assert!(change.micros_per_beat > 0);
            let previous = segments.last().unwrap();
            assert!(change.at_tick >= previous.tick);
            let micros = previous.micros.saturating_add(narrow(
                (change.at_tick - previous.tick) as u128 * previous.micros_per_beat as u128
                    / ticks_per_beat as u128,
            ));
            if change.at_tick == previous.tick {
                segments.pop();
            }
            segments.push(Segment {
                tick: change.at_tick,
                micros,
                micros_per_beat: change.micros_per_beat,
            });
        }
        Self {
            ticks_per_beat,
            segments,
        }
    }

    pub fn micros_at_tick(&self, tick: u64) -> u64 {
        let index = self.segments.partition_point(|s| s.tick <= tick) - 1;
        let segment = &self.segments[index];
        segment.micros.saturating_add(narrow(
            (tick - segment.tick) as u128 * segment.micros_per_beat as u128
                / self.ticks_per_beat as u128,
        ))
    }

    pub fn tick_at_micros(&self, micros: u64, total_ticks: u64) -> u64 {
        if micros >= self.micros_at_tick(total_ticks) {
            return total_ticks;
        }
        let index = self.segments.partition_point(|s| s.micros <= micros) - 1;
        let segment = &self.segments[index];
        // Match the old inverse's integer rounding: return the last tick
        // whose floored microsecond timestamp is <= the requested time.
        let offset = narrow(
            (((micros - segment.micros) as u128 + 1) * self.ticks_per_beat as u128)
                .div_ceil(segment.micros_per_beat as u128)
                - 1,
        );
        let next_tick = self
            .segments
            .get(index + 1)
            .map_or(total_ticks, |next| next.tick.saturating_sub(1));
        segment
            .tick
            .saturating_add(offset)
            .min(next_tick)
            .min(total_ticks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi::tick_to_micros_abs;

    fn inverse_reference(changes: &[TempoChange], tpb: u16, micros: u64, total: u64) -> u64 {
        let (mut low, mut high) = (0, total);
        while low < high {
            let mid = low + (high - low).div_ceil(2);
            if tick_to_micros_abs(mid, changes, tpb) <= micros {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        low
    }

    #[test]
    fn indexed_clock_preserves_rounding_at_tempo_boundaries() {
        for tpb in [1, 96, 480, 960] {
            let changes = vec![
                TempoChange {
                    at_tick: 0,
                    micros_per_beat: 500_000,
                },
                TempoChange {
                    at_tick: 101,
                    micros_per_beat: 333_333,
                },
                TempoChange {
                    at_tick: 101,
                    micros_per_beat: 1,
                },
                TempoChange {
                    at_tick: 250,
                    micros_per_beat: 700_001,
                },
            ];
            let clock = TempoClock::new(&changes, tpb);
            for tick in 0..600 {
                let micros = tick_to_micros_abs(tick, &changes, tpb);
                assert_eq!(clock.micros_at_tick(tick), micros);
                for target in [micros.saturating_sub(1), micros, micros + 1] {
                    assert_eq!(
                        clock.tick_at_micros(target, 600),
                        inverse_reference(&changes, tpb, target, 600)
                    );
                }
            }
        }
    }

    #[test]
    fn implicit_default_and_extreme_timestamps_are_safe() {
        let clock = TempoClock::new(&[], 480);
        assert_eq!(clock.micros_at_tick(480), 500_000);
        assert_eq!(clock.micros_at_tick(u64::MAX), u64::MAX);
        assert_eq!(clock.tick_at_micros(u64::MAX, u64::MAX), u64::MAX);
        assert_eq!(clock.tick_at_micros(0, 0), 0);
    }

    #[test]
    #[ignore = "manual timing comparison"]
    fn measure_tempo_queries() {
        use std::{hint::black_box, time::Instant};
        let changes: Vec<_> = (0..10_000)
            .map(|i| TempoChange {
                at_tick: i * 480,
                micros_per_beat: if i % 2 == 0 { 500_000 } else { 333_333 },
            })
            .collect();
        let clock = TempoClock::new(&changes, 480);
        let ticks: Vec<_> = (0..1_000).map(|i| 4_000_000 + i * 10).collect();
        let start = Instant::now();
        let reference: Vec<_> = ticks
            .iter()
            .map(|&t| tick_to_micros_abs(t, black_box(&changes), 480))
            .collect();
        let scan_time = start.elapsed();
        let start = Instant::now();
        let indexed: Vec<_> = ticks
            .iter()
            .map(|&t| black_box(&clock).micros_at_tick(t))
            .collect();
        let index_time = start.elapsed();
        assert_eq!(reference, indexed);
        println!(
            "10,000 tempo changes / 1,000 queries: scan {scan_time:?}, indexed {index_time:?}"
        );
    }
}
