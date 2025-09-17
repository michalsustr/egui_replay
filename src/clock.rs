//! This module provides utilities for time measurement and management.
//!
//! It includes:
//! - A `Clock` trait for abstracting time providers.
//! - `SystemClock`: A `Clock` implementation using the system's real-time
//!   clock.
//! - `ManualClock`: A mockable `Clock` implementation that allows manual
//!   advancement of time, useful for testing time-dependent logic.
//! - `Stopwatch`: A utility to measure elapsed time using a `Clock`.
//! - `Timer`: A utility built upon `Stopwatch` to check if a specific duration
//!   has elapsed (timeout).
//!
//! TODO #217: add monotonic clock

use std::fmt;

use crate::timestamp::{NanoDelta, NanoTimestamp};

/// A trait for providing the current time.
pub trait Clock: Send + Sync {
    fn now(&self) -> NanoTimestamp;
}

/// A time provider that uses the system's clock.
#[derive(Clone, Debug)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> NanoTimestamp {
        // We use chrono here as it is platform agnostic.
        NanoTimestamp::try_from(chrono::Utc::now()).unwrap()
    }
}

use std::sync::{Arc, Mutex};

/// A time provider that can be mocked to advance time.
#[derive(Clone, Debug, Default)]
pub struct ManualClock {
    current_time: Arc<Mutex<NanoTimestamp>>,
}

impl ManualClock {
    pub fn new() -> Self {
        let zero_time = NanoTimestamp::zero();
        Self {
            current_time: Arc::new(Mutex::new(zero_time)),
        }
    }

    pub fn advance_by(&self, duration: NanoDelta) {
        assert!(duration > NanoDelta::zero());
        let mut time = self.current_time.lock().unwrap();
        *time = *time + duration;
    }

    pub fn advance_to(&self, time: NanoTimestamp) {
        let mut current_time = self.current_time.lock().unwrap();
        *current_time = time;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> NanoTimestamp {
        *self.current_time.lock().unwrap()
    }
}

/// Measure elapsed time.
pub struct Stopwatch {
    clock: Box<dyn Clock>,
    start_time: NanoTimestamp,
}

impl fmt::Debug for Stopwatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Stopwatch")
            .field("clock", &self.clock.now()) // Just show a placeholder
            .field("start_time", &self.start_time)
            .finish()
    }
}

impl Stopwatch {
    pub fn new(clock: Box<dyn Clock>) -> Self {
        Self {
            start_time: clock.now(),
            clock,
        }
    }

    pub fn elapsed(&self) -> NanoDelta {
        self.clock.now() - self.start_time
    }

    pub fn reset(&mut self) {
        self.start_time = self.clock.now();
    }
}

/// A timer that can be used to measure the elapsed time and check if timeout
/// has occurred.
#[derive(Debug)]
pub struct Timer {
    stopwatch: Stopwatch,
    duration: NanoDelta,
}

impl Timer {
    pub fn new(clock: Box<dyn Clock>, duration: NanoDelta) -> Self {
        Self {
            duration,
            stopwatch: Stopwatch::new(clock),
        }
    }

    pub fn is_timeout(&self) -> bool {
        self.stopwatch.elapsed() >= self.duration
    }

    pub fn elapsed(&self) -> NanoDelta {
        self.stopwatch.elapsed()
    }

    pub fn reset(&mut self) {
        self.stopwatch.reset();
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn advance_time_in_manual_clock() {
        // Arrange
        struct Component {
            times: Vec<NanoTimestamp>,
            provider: Box<dyn Clock>,
        }

        impl Component {
            fn new(provider: Box<dyn Clock>) -> Self {
                Self {
                    times: Vec::new(),
                    provider,
                }
            }
            fn append_now(&mut self) {
                self.times.push(self.provider.now());
            }
        }

        let clock = ManualClock::new();
        let mut a = Component::new(Box::new(clock.clone()));
        let mut b = Component::new(Box::new(clock.clone()));

        let expected_a_times_nanos: Vec<i64> = vec![0, 1, 3, 4];
        let expected_b_times_nanos: Vec<i64> = vec![1, 4];

        // Act
        a.append_now(); // t=0
        clock.advance_by(NanoDelta::from_nanos(1)); // t=0 -> t=1
        a.append_now(); // t=1
        b.append_now(); // t=1
        clock.advance_by(NanoDelta::from_nanos(2)); // t=1 -> t=3
        a.append_now(); // t=3
        clock.advance_by(NanoDelta::from_nanos(1)); // t=3 -> t=4
        a.append_now(); // t=4
        b.append_now(); // t=4

        let actual_a_times_nanos = a.times.iter().map(|t| t.as_nanos()).collect::<Vec<_>>();
        let actual_b_times_nanos = b.times.iter().map(|t| t.as_nanos()).collect::<Vec<_>>();

        // Assert
        assert_eq!(actual_a_times_nanos, expected_a_times_nanos);
        assert_eq!(actual_b_times_nanos, expected_b_times_nanos);
    }

    #[test]
    fn advance_time_across_threads_simplified() {
        // Arrange
        use std::sync::{mpsc::sync_channel, Arc, Barrier};

        let clock = ManualClock::new();
        let worker_count = 4;
        let steps = 4;

        // A barrier that synchronizes the main thread plus all worker threads.
        // Use two-phase barrier synchronization.
        let barrier = Arc::new(Barrier::new(worker_count + 1));
        let (sender, receiver) = sync_channel(worker_count);

        for _ in 0..worker_count {
            let clock_clone = Box::new(clock.clone());
            let barrier_clone = barrier.clone();
            let sender_clone = sender.clone();

            std::thread::spawn(move || {
                let mut times = Vec::with_capacity(steps);
                for _ in 0..steps {
                    barrier_clone.wait(); // phase one: ensure that the shared state is ready.
                    times.push(clock_clone.now()); // record the current time
                    barrier_clone.wait(); // phase two: ensure that all threads have completed their work.
                }
                sender_clone.send(times).unwrap();
            });
        }

        let expected_times_per_thread: Vec<NanoTimestamp> = (1..=steps)
            .map(|i| NanoTimestamp::from_nanos(i as i64))
            .collect();

        // Act
        // Main thread advances clock and synchronizes
        for _ in 0..steps {
            clock.advance_by(NanoDelta::from_nanos(1));
            barrier.wait(); // let worker threads read the updated time
            barrier.wait(); // wait until they finish recording before next iteration
        }

        // Collect results
        let actual_results: Vec<Vec<NanoTimestamp>> = (0..worker_count)
            .map(|_| receiver.recv().unwrap())
            .collect();

        // Assert
        for actual_thread_times in actual_results {
            assert_eq!(
                actual_thread_times, expected_times_per_thread,
                "All thread components should show consistent time steps"
            );
        }
    }

    #[test]
    fn stopwatch_new_and_elapsed_initial() {
        // Arrange
        let clock = ManualClock::new();
        let stopwatch = Stopwatch::new(Box::new(clock.clone()));
        let expected_elapsed = NanoDelta::zero();

        // Act
        let actual_elapsed = stopwatch.elapsed();

        // Assert
        assert_eq!(actual_elapsed, expected_elapsed);
    }

    #[test]
    fn stopwatch_elapsed_after_time_passes() {
        // Arrange
        let clock = ManualClock::new();
        let stopwatch = Stopwatch::new(Box::new(clock.clone()));
        let advance_duration = NanoDelta::from(5);
        let expected_elapsed = advance_duration;

        // Act
        clock.advance_by(advance_duration);
        let actual_elapsed = stopwatch.elapsed();

        // Assert
        assert_eq!(actual_elapsed, expected_elapsed);
    }

    #[test]
    fn stopwatch_reset() {
        // Arrange
        let clock = ManualClock::new();
        let mut stopwatch = Stopwatch::new(Box::new(clock.clone()));
        let first_duration = NanoDelta::from(3);
        let second_duration = NanoDelta::from(7);

        // Act & Assert for first period
        clock.advance_by(first_duration);
        let actual_elapsed_before_reset = stopwatch.elapsed();
        assert_eq!(actual_elapsed_before_reset, first_duration);

        // Act: Reset the stopwatch
        stopwatch.reset();
        let actual_elapsed_immediately_after_reset = stopwatch.elapsed();
        assert_eq!(
            actual_elapsed_immediately_after_reset,
            NanoDelta::zero(),
            "Elapsed should be zero immediately after reset"
        );

        // Act & Assert for second period
        clock.advance_by(second_duration);
        let actual_elapsed_after_reset_and_advance = stopwatch.elapsed();

        // Assert
        assert_eq!(
            actual_elapsed_after_reset_and_advance, second_duration,
            "Elapsed after reset should only measure from the reset point"
        );
    }

    #[test]
    fn timer_new_and_initial_state() {
        // Arrange
        let clock = ManualClock::new();
        let duration = NanoDelta::from(10);
        let timer = Timer::new(Box::new(clock.clone()), duration);
        let expected_elapsed_initial = NanoDelta::zero();

        // Act
        let actual_is_timeout_initial = timer.is_timeout();
        let actual_elapsed_initial = timer.elapsed();

        // Assert
        assert!(
            !actual_is_timeout_initial,
            "Timer should not be timed out initially"
        );
        assert_eq!(
            actual_elapsed_initial, expected_elapsed_initial,
            "Timer elapsed should be zero initially"
        );
        assert_eq!(
            timer.duration, duration,
            "Timer should store the correct duration"
        );
    }

    #[test]
    fn timer_is_timeout() {
        // Arrange
        let clock = ManualClock::new();
        let duration = NanoDelta::from(10);
        let timer = Timer::new(Box::new(clock.clone()), duration);

        // Act & Assert: Before duration
        clock.advance_by(NanoDelta::from(5)); // Advance by 5ns, total 5ns
        let actual_is_timeout_before = timer.is_timeout();
        let actual_elapsed_before = timer.elapsed();
        assert!(
            !actual_is_timeout_before,
            "Timer should not be timed out before duration passes"
        );
        assert_eq!(actual_elapsed_before, NanoDelta::from(5));

        // Act & Assert: Exactly at duration
        clock.advance_by(NanoDelta::from(5)); // Advance by 5ns, total 10ns
        let actual_is_timeout_at = timer.is_timeout();
        let actual_elapsed_at = timer.elapsed();
        assert!(
            actual_is_timeout_at,
            "Timer should be timed out exactly at duration"
        );
        assert_eq!(actual_elapsed_at, NanoDelta::from(10));

        // Act & Assert: After duration
        clock.advance_by(NanoDelta::from(5)); // Advance by 5ns, total 15ns
        let actual_is_timeout_after = timer.is_timeout();
        let actual_elapsed_after = timer.elapsed();
        assert!(
            actual_is_timeout_after,
            "Timer should be timed out after duration passes"
        );
        assert_eq!(actual_elapsed_after, NanoDelta::from(15));
    }

    #[test]
    fn timer_elapsed() {
        // Arrange
        let clock = ManualClock::new();
        let duration = NanoDelta::from(10);
        let timer = Timer::new(Box::new(clock.clone()), duration);
        let advance_duration = NanoDelta::from(3);
        let expected_elapsed = advance_duration;

        // Act
        clock.advance_by(advance_duration);
        let actual_elapsed = timer.elapsed();

        // Assert
        assert_eq!(actual_elapsed, expected_elapsed);
    }

    #[test]
    fn timer_reset() {
        // Arrange
        let clock = ManualClock::new();
        let duration = NanoDelta::from(5);
        let mut timer = Timer::new(Box::new(clock.clone()), duration);

        // Act & Assert: Timeout the timer
        clock.advance_by(NanoDelta::from(6)); // Total 6ns, timeout
        assert!(timer.is_timeout(), "Timer should be timed out before reset");
        assert_eq!(timer.elapsed(), NanoDelta::from(6));

        // Act: Reset the timer
        timer.reset();
        let expected_elapsed_after_reset = NanoDelta::zero();

        // Assert: State after reset
        assert!(
            !timer.is_timeout(),
            "Timer should not be timed out immediately after reset"
        );
        assert_eq!(
            timer.elapsed(),
            expected_elapsed_after_reset,
            "Timer elapsed should be zero after reset"
        );

        // Act & Assert: Behavior after reset
        clock.advance_by(NanoDelta::from(3)); // Advance by 3ns (from reset point)
        assert!(
            !timer.is_timeout(),
            "Timer should not be timed out before duration passes after reset"
        );
        assert_eq!(timer.elapsed(), NanoDelta::from(3));

        clock.advance_by(NanoDelta::from(2)); // Advance by 2ns, total 5ns from reset
        assert!(
            timer.is_timeout(),
            "Timer should be timed out at duration after reset"
        );
        assert_eq!(timer.elapsed(), NanoDelta::from(5));
    }
}
