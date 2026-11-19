use crate::{
    CommandAction, CommandError, CommandOutcome, Country, CountryId, CountryTag, PlayerId,
    Province, ProvinceId, QueuedCommand, StateValidationError, Tick, WorldState,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Simulation {
    pub(crate) state: WorldState,
    pub(crate) queue: Vec<QueuedCommand>,
}

impl Simulation {
    pub fn new(seed: u64) -> Self {
        Self {
            state: WorldState::new(seed),
            queue: Vec::new(),
        }
    }

    pub fn state(&self) -> &WorldState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut WorldState {
        &mut self.state
    }

    /// Queues an input for a later tick. Arrival order does not determine
    /// execution order; commands are sorted by player ID and sequence.
    pub fn schedule_command(
        &mut self,
        command: QueuedCommand,
    ) -> Result<(), QueueError> {
        if command.tick <= self.state.tick {
            return Err(QueueError::TickNotInFuture {
                current: self.state.tick,
                requested: command.tick,
            });
        }

        let duplicate = self.queue.iter().any(|queued| {
            queued.tick == command.tick
                && queued.player_id == command.player_id
                && queued.sequence == command.sequence
        });
        if duplicate {
            return Err(QueueError::DuplicateOrder {
                tick: command.tick,
                player_id: command.player_id,
                sequence: command.sequence,
            });
        }

        self.queue.push(command);
        Ok(())
    }

    /// Advances exactly one deterministic step and applies commands scheduled
    /// for that step.
    pub fn step(&mut self) -> Result<Vec<CommandOutcome>, ClockError> {
        let next_tick = Tick(self.state.tick.0.checked_add(1).ok_or(ClockError::TickOverflow)?);
        self.state.tick = next_tick;

        let mut due = Vec::new();
        let mut future = Vec::with_capacity(self.queue.len());
        for command in self.queue.drain(..) {
            if command.tick <= next_tick {
                due.push(command);
            } else {
                future.push(command);
            }
        }
        self.queue = future;
        due.sort_by_key(|command| (command.player_id, command.sequence));

        Ok(due
            .into_iter()
            .map(|command| {
                let result = self.apply(command.action);
                CommandOutcome {
                    tick: next_tick,
                    player_id: command.player_id,
                    sequence: command.sequence,
                    result,
                }
            })
            .collect())
    }

    fn apply(&mut self, action: CommandAction) -> Result<(), CommandError> {
        match action {
            CommandAction::AddCountry { country_id, tag } => {
                if self.state.countries.contains_key(&country_id) {
                    return Err(CommandError::DuplicateCountry(country_id));
                }
                if self.state.countries.values().any(|country| country.tag == tag) {
                    return Err(CommandError::DuplicateCountryTag(tag));
                }
                self.state.countries.insert(
                    country_id,
                    Country {
                        tag,
                        stability_bp: 0,
                    },
                );
                Ok(())
            }
            CommandAction::AddProvince { province_id, owner } => {
                if self.state.provinces.contains_key(&province_id) {
                    return Err(CommandError::DuplicateProvince(province_id));
                }
                if let Some(country_id) = owner {
                    if !self.state.countries.contains_key(&country_id) {
                        return Err(CommandError::UnknownCountry(country_id));
                    }
                }
                self.state
                    .provinces
                    .insert(province_id, Province { owner });
                Ok(())
            }
            CommandAction::TransferProvince {
                province_id,
                new_owner,
            } => {
                if let Some(country_id) = new_owner {
                    if !self.state.countries.contains_key(&country_id) {
                        return Err(CommandError::UnknownCountry(country_id));
                    }
                }
                let province = self
                    .state
                    .provinces
                    .get_mut(&province_id)
                    .ok_or(CommandError::UnknownProvince(province_id))?;
                province.owner = new_owner;
                Ok(())
            }
            CommandAction::SetCountryStability {
                country_id,
                stability_bp,
            } => {
                if !(-10_000..=10_000).contains(&stability_bp) {
                    return Err(CommandError::StabilityOutOfRange(stability_bp));
                }
                let country = self
                    .state
                    .countries
                    .get_mut(&country_id)
                    .ok_or(CommandError::UnknownCountry(country_id))?;
                country.stability_bp = stability_bp;
                Ok(())
            }
        }
    }

    pub(crate) fn validate_loaded(&self) -> Result<(), LoadValidationError> {
        self.state.validate().map_err(LoadValidationError::State)?;

        let mut orders = BTreeSet::new();
        for command in &self.queue {
            if command.tick <= self.state.tick {
                return Err(LoadValidationError::CommandNotInFuture(command.tick));
            }
            let key = (command.tick, command.player_id, command.sequence);
            if !orders.insert(key) {
                return Err(LoadValidationError::DuplicateCommandOrder {
                    tick: command.tick,
                    player_id: command.player_id,
                    sequence: command.sequence,
                });
            }
            if let CommandAction::AddCountry { tag, .. } = &command.action {
                CountryTag::new(tag.as_str())
                    .map_err(|_| LoadValidationError::InvalidQueuedCountryTag)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockError {
    TickOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueueError {
    TickNotInFuture {
        current: Tick,
        requested: Tick,
    },
    DuplicateOrder {
        tick: Tick,
        player_id: PlayerId,
        sequence: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum LoadValidationError {
    State(StateValidationError),
    CommandNotInFuture(Tick),
    DuplicateCommandOrder {
        tick: Tick,
        player_id: PlayerId,
        sequence: u64,
    },
    InvalidQueuedCountryTag,
}
