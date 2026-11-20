//! Deterministic conversion from integer ticks to a configurable game calendar.

use crate::Tick;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CalendarConfig {
    pub epoch_tick: Tick,
    pub start_year: i64,
    /// One-based month within the repeated `days_per_month` cycle.
    pub start_month: u16,
    /// One-based day within `start_month`.
    pub start_day: u16,
    /// Zero-based hour within the day.
    pub start_hour: u16,
    pub ticks_per_hour: u32,
    pub hours_per_day: u16,
    /// A repeated year cycle; entries define each month's number of days.
    pub days_per_month: Vec<u16>,
}

impl Default for CalendarConfig {
    fn default() -> Self {
        Self {
            epoch_tick: Tick(0),
            start_year: 1,
            start_month: 1,
            start_day: 1,
            start_hour: 0,
            ticks_per_hour: 1,
            hours_per_day: 24,
            days_per_month: vec![30; 12],
        }
    }
}

impl CalendarConfig {
    pub fn validate(&self) -> Result<(), CalendarError> {
        if self.ticks_per_hour == 0
            || self.hours_per_day == 0
            || self.days_per_month.is_empty()
            || self.days_per_month.len() > 256
            || self.days_per_month.contains(&0)
            || self.start_month == 0
            || usize::from(self.start_month) > self.days_per_month.len()
            || self.start_day == 0
            || self.start_day > self.days_per_month[usize::from(self.start_month - 1)]
            || self.start_hour >= self.hours_per_day
        {
            return Err(CalendarError::InvalidConfig);
        }
        Ok(())
    }

    pub fn date_at(&self, tick: Tick) -> Result<CalendarDate, CalendarError> {
        self.validate()?;
        if tick < self.epoch_tick {
            return Err(CalendarError::BeforeEpoch {
                epoch: self.epoch_tick,
                tick,
            });
        }
        let elapsed_ticks = tick.0 - self.epoch_tick.0;
        let elapsed_hours = elapsed_ticks / u64::from(self.ticks_per_hour);
        let tick_in_hour = u32::try_from(elapsed_ticks % u64::from(self.ticks_per_hour))
            .map_err(|_| CalendarError::ArithmeticOverflow)?;
        let absolute_hour = u64::from(self.start_hour)
            .checked_add(elapsed_hours)
            .ok_or(CalendarError::ArithmeticOverflow)?;
        let elapsed_days = absolute_hour / u64::from(self.hours_per_day);
        let hour = u16::try_from(absolute_hour % u64::from(self.hours_per_day))
            .map_err(|_| CalendarError::ArithmeticOverflow)?;

        let year_length = self
            .days_per_month
            .iter()
            .try_fold(0_u64, |sum, days| sum.checked_add(u64::from(*days)))
            .ok_or(CalendarError::ArithmeticOverflow)?;
        let start_day_of_year = self.days_per_month[..usize::from(self.start_month - 1)]
            .iter()
            .try_fold(u64::from(self.start_day - 1), |sum, days| {
                sum.checked_add(u64::from(*days))
            })
            .ok_or(CalendarError::ArithmeticOverflow)?;
        let total_day = start_day_of_year
            .checked_add(elapsed_days)
            .ok_or(CalendarError::ArithmeticOverflow)?;
        let year_offset = total_day / year_length;
        let year = i64::try_from(year_offset)
            .ok()
            .and_then(|offset| self.start_year.checked_add(offset))
            .ok_or(CalendarError::ArithmeticOverflow)?;
        let mut day_of_year = total_day % year_length;
        let mut month = 0_u16;
        for (index, days) in self.days_per_month.iter().enumerate() {
            if day_of_year < u64::from(*days) {
                month = u16::try_from(index + 1).map_err(|_| CalendarError::ArithmeticOverflow)?;
                break;
            }
            day_of_year -= u64::from(*days);
        }
        let day = u16::try_from(day_of_year + 1).map_err(|_| CalendarError::ArithmeticOverflow)?;
        if month == 0 {
            return Err(CalendarError::InvalidConfig);
        }
        Ok(CalendarDate {
            year,
            month,
            day,
            hour,
            tick_in_hour,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CalendarDate {
    pub year: i64,
    pub month: u16,
    pub day: u16,
    pub hour: u16,
    pub tick_in_hour: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CalendarError {
    InvalidConfig,
    BeforeEpoch { epoch: Tick, tick: Tick },
    ArithmeticOverflow,
}

impl fmt::Display for CalendarError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig => formatter.write_str("invalid calendar configuration"),
            Self::BeforeEpoch { epoch, tick } => write!(
                formatter,
                "tick {} precedes calendar epoch {}",
                tick.0, epoch.0
            ),
            Self::ArithmeticOverflow => formatter.write_str("calendar arithmetic overflow"),
        }
    }
}

impl std::error::Error for CalendarError {}
