//! Frame timing (three.js `Clock` and `Timer`). The host gives the time in
//! milliseconds (`now`), so the crate needs no clock of its own.

use ranger::prelude::*;

/// Seconds since start and between calls, started on first use when
/// `auto_start` is set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clock {
    pub auto_start: bool,
    pub start_time: f64,
    pub old_time: f64,
    pub elapsed_time: f64,
    pub running: bool,
}

impl Clock {
    pub fn new(auto_start: bool) -> Clock {
        Clock { auto_start, start_time: 0.0, old_time: 0.0, elapsed_time: 0.0, running: false }
    }

    pub fn start(&mut self, now: f64) {
        self.start_time = now;
        self.old_time = now;
        self.elapsed_time = 0.0;
        self.running = true;
    }

    pub fn stop(&mut self, now: f64) {
        self.get_elapsed_time(now);
        self.running = false;
        self.auto_start = false;
    }

    pub fn get_elapsed_time(&mut self, now: f64) -> f64 {
        self.get_delta(now);
        self.elapsed_time
    }

    pub fn get_delta(&mut self, now: f64) -> f64 {
        if self.auto_start && !self.running {
            self.start(now);
            return 0.0;
        }
        let mut diff = 0.0;
        if self.running {
            diff = (now - self.old_time) / 1000.0;
            self.old_time = now;
            self.elapsed_time += diff;
        }
        diff
    }
}

/// three.js's `Timer`: updated once per frame, scaled deltas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timer {
    pub previous_time: f64,
    pub current_time: f64,
    pub start_time: f64,
    pub delta: f64,
    pub elapsed: f64,
    pub timescale: f64,
}

impl Timer {
    pub fn new(now: f64) -> Timer {
        Timer { previous_time: 0.0, current_time: 0.0, start_time: now, delta: 0.0, elapsed: 0.0, timescale: 1.0 }
    }

    pub fn get_delta(&self) -> f64 {
        self.delta / 1000.0
    }

    pub fn get_elapsed(&self) -> f64 {
        self.elapsed / 1000.0
    }

    pub fn get_timescale(&self) -> f64 {
        self.timescale
    }

    pub fn set_timescale(&mut self, timescale: f64) {
        self.timescale = timescale;
    }

    pub fn reset(&mut self, now: f64) {
        self.current_time = now - self.start_time;
    }

    /// A new frame at `timestamp` (ms, on the clock `new` was given).
    pub fn update(&mut self, timestamp: f64) {
        self.previous_time = self.current_time;
        self.current_time = timestamp - self.start_time;
        self.delta = (self.current_time - self.previous_time) * self.timescale;
        self.elapsed += self.delta;
    }
}
