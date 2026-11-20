//! Wall-clock pacing for the fixed-step simulation.
//!
//! This type converts elapsed host time into whole simulation ticks. It is not
//! part of authoritative state and is intentionally absent from save files.

use std::time::Duration;

const SPEED_SCALE: u128 = 1_000_000;
const NANOS_PER_SECOND: u128 = 1_000_000_000;
const TICK_UNITS: u128 = NANOS_PER_SECOND * SPEED_SCALE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacerError {
    ZeroTickRate,
    ZeroSpeed,
    ArithmeticOverflow,
}

/// Integer fixed-step pacing. `speed_ppm = 1_000_000` means normal speed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixedStepClock {
    tick_rate_hz: u32,
    speed_ppm: u32,
    paused: bool,
    accumulated_units: u128,
}

impl FixedStepClock {
    pub fn new(tick_rate_hz: u32) -> Result<Self, PacerError> {
        if tick_rate_hz == 0 {
            return Err(PacerError::ZeroTickRate);
        }
        Ok(Self {
            tick_rate_hz,
            speed_ppm: SPEED_SCALE as u32,
            paused: false,
            accumulated_units: 0,
        })
    }

    pub fn tick_rate_hz(&self) -> u32 {
        self.tick_rate_hz
    }

    pub fn speed_ppm(&self) -> u32 {
        self.speed_ppm
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// Sets a positive integer speed multiplier in parts per million.
    /// For example, 2_000_000 is 2x and 250_000 is 0.25x.
    pub fn set_speed_ppm(&mut self, speed_ppm: u32) -> Result<(), PacerError> {
        if speed_ppm == 0 {
            return Err(PacerError::ZeroSpeed);
        }
        self.speed_ppm = speed_ppm;
        Ok(())
    }

    /// Adds elapsed host time. Paused time is discarded, so unpausing never
    /// causes the engine to catch up for time spent in the pause state.
    pub fn accumulate(&mut self, elapsed: Duration) -> Result<(), PacerError> {
        if self.paused {
            return Ok(());
        }
        let added = elapsed
            .as_nanos()
            .checked_mul(u128::from(self.tick_rate_hz))
            .and_then(|value| value.checked_mul(u128::from(self.speed_ppm)))
            .ok_or(PacerError::ArithmeticOverflow)?;
        self.accumulated_units = self
            .accumulated_units
            .checked_add(added)
            .ok_or(PacerError::ArithmeticOverflow)?;
        Ok(())
    }

    /// Returns the number of whole ticks waiting to be simulated.
    pub fn due_ticks(&self) -> u128 {
        self.accumulated_units / TICK_UNITS
    }

    /// Removes up to `limit` due ticks. Any remaining backlog is retained,
    /// allowing the host to keep rendering between bounded simulation batches.
    pub fn take_due_ticks(&mut self, limit: u32) -> u32 {
        let taken = self.due_ticks().min(u128::from(limit)) as u32;
        self.accumulated_units -= u128::from(taken) * TICK_UNITS;
        taken
    }
}
