//! Hybrid turn structure: timed "days" instead of "End turn".
//!
//! Every participant acts in real time, at the same time, but only within a daily budget
//! ([`ActionBudget`]: movement points plus actions). A day lasts [`DayClock::day_length`] seconds;
//! when it runs out — or everyone has declared themselves ready — a new day starts and every
//! budget refills. Nobody waits for the slowest player, yet the game keeps the pacing and
//! planning of a turn-based game. Works for single player, hot seat or online (the host runs the
//! clock).

use std::collections::HashSet;

/// Per-participant resources for one day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionBudget {
    /// Movement points per day and left today.
    pub moves_per_day: u32,
    pub moves: u32,
    /// Other actions (attack, cast, build, trade...) per day and left today.
    pub actions_per_day: u32,
    pub actions: u32,
}

impl ActionBudget {
    pub fn new(moves_per_day: u32, actions_per_day: u32) -> Self {
        ActionBudget { moves_per_day, moves: moves_per_day, actions_per_day, actions: actions_per_day }
    }

    pub fn refill(&mut self) {
        self.moves = self.moves_per_day;
        self.actions = self.actions_per_day;
    }

    /// Pay `n` movement points if affordable.
    pub fn spend_moves(&mut self, n: u32) -> bool {
        if self.moves >= n {
            self.moves -= n;
            true
        } else {
            false
        }
    }

    /// Use one action if any are left.
    pub fn spend_action(&mut self) -> bool {
        if self.actions > 0 {
            self.actions -= 1;
            true
        } else {
            false
        }
    }

    /// Nothing left to do today (no actions and no movement).
    pub fn exhausted(&self) -> bool {
        self.actions == 0 && self.moves == 0
    }
}

/// The day timer.
#[derive(Clone, Debug)]
pub struct DayClock {
    /// Seconds per day.
    pub day_length: f32,
    /// Current day, starting at 1.
    pub day: u32,
    elapsed: f32,
    pub paused: bool,
    ready: HashSet<u64>,
}

impl DayClock {
    pub fn new(day_length: f32) -> Self {
        DayClock { day_length, day: 1, elapsed: 0.0, paused: false, ready: HashSet::new() }
    }

    pub fn time_left(&self) -> f32 {
        (self.day_length - self.elapsed).max(0.0)
    }

    /// 0 at dawn, 1 at the end of the day.
    pub fn progress(&self) -> f32 {
        (self.elapsed / self.day_length.max(0.001)).clamp(0.0, 1.0)
    }

    /// Declare a participant done for today (they can still act until the day ends).
    pub fn set_ready(&mut self, who: u64, ready: bool) {
        if ready {
            self.ready.insert(who);
        } else {
            self.ready.remove(&who);
        }
    }

    pub fn is_ready(&self, who: u64) -> bool {
        self.ready.contains(&who)
    }

    pub fn ready_count(&self) -> usize {
        self.ready.len()
    }

    /// Advance time. Returns the new day number when a day ends — because time ran out or all
    /// `participants` are ready. Budgets should be refilled then.
    pub fn update(&mut self, dt: f32, participants: &[u64]) -> Option<u32> {
        if self.paused {
            return None;
        }
        self.elapsed += dt;
        let all_ready = !participants.is_empty() && participants.iter().all(|p| self.ready.contains(p));
        if self.elapsed >= self.day_length || all_ready {
            self.next_day();
            return Some(self.day);
        }
        None
    }

    /// Start the next day now.
    pub fn next_day(&mut self) {
        self.day += 1;
        self.elapsed = 0.0;
        self.ready.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_end_on_time_or_when_everyone_is_ready() {
        let mut c = DayClock::new(10.0);
        assert_eq!(c.update(4.0, &[1, 2]), None);
        assert!((c.time_left() - 6.0).abs() < 1e-5);
        assert_eq!(c.update(6.5, &[1, 2]), Some(2));
        c.set_ready(1, true);
        assert_eq!(c.update(0.1, &[1, 2]), None);
        c.set_ready(2, true);
        assert_eq!(c.update(0.1, &[1, 2]), Some(3));
        assert_eq!(c.ready_count(), 0);
    }

    #[test]
    fn budgets() {
        let mut b = ActionBudget::new(10, 1);
        assert!(b.spend_moves(7));
        assert!(!b.spend_moves(4));
        assert!(b.spend_action());
        assert!(!b.spend_action());
        b.spend_moves(3);
        assert!(b.exhausted());
        b.refill();
        assert_eq!((b.moves, b.actions), (10, 1));
    }
}
