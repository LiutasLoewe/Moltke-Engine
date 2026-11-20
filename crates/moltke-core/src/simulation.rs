use crate::{
    CalendarConfig, CalendarDate, CalendarError, CommandAction, CommandError, CommandOutcome,
    Country, CountryTag, ModifierSet, PlayerId, Province, QueuedCommand, StatId,
    StateValidationError, Tick, WorldState,
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

    pub fn calendar_date(&self) -> Result<CalendarDate, CalendarError> {
        self.state.calendar_config().date_at(self.state.tick())
    }

    pub fn set_calendar_config(&mut self, config: CalendarConfig) -> Result<(), CalendarError> {
        config.validate()?;
        self.state.calendar = config;
        Ok(())
    }

    /// Commands still waiting for their scheduled tick, in arrival order.
    /// Replay and diagnostic tools should sort by the canonical command key.
    pub fn queued_commands(&self) -> &[QueuedCommand] {
        &self.queue
    }

    /// Queues an input for a later tick. Arrival order does not determine
    /// execution order; commands are sorted by player ID and sequence.
    pub fn schedule_command(&mut self, command: QueuedCommand) -> Result<(), QueueError> {
        self.schedule_commands(vec![command])
    }

    /// Atomically schedules a group of commands. This is used by event choices
    /// and scripted effects so a duplicate or past-tick order cannot leave a
    /// partially queued effect list.
    pub fn schedule_commands(&mut self, commands: Vec<QueuedCommand>) -> Result<(), QueueError> {
        let mut batch = BTreeSet::new();
        for command in &commands {
            if command.tick <= self.state.tick {
                return Err(QueueError::TickNotInFuture {
                    current: self.state.tick,
                    requested: command.tick,
                });
            }
            let key = (command.tick, command.player_id, command.sequence);
            if !batch.insert(key)
                || self.queue.iter().any(|queued| {
                    queued.tick == command.tick
                        && queued.player_id == command.player_id
                        && queued.sequence == command.sequence
                })
            {
                return Err(QueueError::DuplicateOrder {
                    tick: command.tick,
                    player_id: command.player_id,
                    sequence: command.sequence,
                });
            }
        }
        self.queue.extend(commands);
        Ok(())
    }

    /// Advances exactly one deterministic step and applies commands scheduled
    /// for that step.
    pub fn step(&mut self) -> Result<Vec<CommandOutcome>, ClockError> {
        let next_tick = Tick(
            self.state
                .tick
                .0
                .checked_add(1)
                .ok_or(ClockError::TickOverflow)?,
        );
        self.state.tick = next_tick;
        self.state.modifiers.remove_expired(next_tick);

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
                let result = self.apply(command.action, command.player_id);
                CommandOutcome {
                    tick: next_tick,
                    player_id: command.player_id,
                    sequence: command.sequence,
                    result,
                }
            })
            .collect())
    }

    fn apply(&mut self, action: CommandAction, player_id: PlayerId) -> Result<(), CommandError> {
        Self::apply_to_state(&mut self.state, action, player_id)
    }

    fn apply_to_state(
        state: &mut WorldState,
        action: CommandAction,
        player_id: PlayerId,
    ) -> Result<(), CommandError> {
        match action {
            CommandAction::AddCountry { country_id, tag } => {
                if state.countries.contains_key(&country_id) {
                    return Err(CommandError::DuplicateCountry(country_id));
                }
                if state.countries.values().any(|country| country.tag == tag) {
                    return Err(CommandError::DuplicateCountryTag(tag));
                }
                state.countries.insert(
                    country_id,
                    Country {
                        tag,
                        stability_bp: 0,
                    },
                );
                Ok(())
            }
            CommandAction::AddProvince { province_id, owner } => {
                if state.provinces.contains_key(&province_id) {
                    return Err(CommandError::DuplicateProvince(province_id));
                }
                if let Some(country_id) = owner
                    && !state.countries.contains_key(&country_id)
                {
                    return Err(CommandError::UnknownCountry(country_id));
                }
                state.provinces.insert(province_id, Province { owner });
                Ok(())
            }
            CommandAction::TransferProvince {
                province_id,
                new_owner,
            } => {
                if let Some(country_id) = new_owner
                    && !state.countries.contains_key(&country_id)
                {
                    return Err(CommandError::UnknownCountry(country_id));
                }
                let province = state
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
                let country = state
                    .countries
                    .get_mut(&country_id)
                    .ok_or(CommandError::UnknownCountry(country_id))?;
                country.stability_bp = stability_bp;
                Ok(())
            }
            CommandAction::SetStat {
                entity,
                stat,
                value,
            } => {
                StatId::new(stat.as_str())
                    .map_err(|_| CommandError::InvalidStatId(stat.as_str().to_owned()))?;
                state.set_base_stat(entity, stat, value);
                Ok(())
            }
            CommandAction::AddModifier { modifier } => state
                .modifiers
                .add(modifier)
                .map_err(CommandError::Modifier),
            CommandAction::RemoveModifier { modifier_id } => state
                .modifiers
                .remove(modifier_id)
                .map(|_| ())
                .map_err(CommandError::Modifier),
            CommandAction::CreateEntity { entity_id } => state
                .entity_store
                .create(entity_id)
                .map_err(CommandError::EntityStore),
            CommandAction::AddEntityTag { entity_id, tag } => state
                .entity_store
                .add_tag(entity_id, tag)
                .map_err(CommandError::EntityStore),
            CommandAction::RemoveEntityTag { entity_id, tag } => state
                .entity_store
                .remove_tag(entity_id, &tag)
                .map_err(CommandError::EntityStore),
            CommandAction::SetComponentField {
                entity_id,
                component,
                field,
                value,
            } => state
                .entity_store
                .set_component_field(entity_id, component, field, value)
                .map_err(CommandError::EntityStore),
            CommandAction::RemoveComponentField {
                entity_id,
                component,
                field,
            } => state
                .entity_store
                .remove_component_field(entity_id, &component, &field)
                .map_err(CommandError::EntityStore),
            CommandAction::OpenEvent {
                event_id,
                recipient,
                once,
                cooldown_ticks,
            } => {
                crate::EventId::new(event_id.as_str()).map_err(CommandError::EventState)?;
                let now = state.tick();
                if !state
                    .event_runtime
                    .can_fire(&event_id, now, once, cooldown_ticks)
                {
                    return Err(CommandError::EventUnavailable(event_id));
                }
                state
                    .event_runtime
                    .open(event_id, recipient, now, once)
                    .map_err(CommandError::EventState)?;
                Ok(())
            }
            CommandAction::ResolveEvent { instance_id } => {
                let pending = state
                    .event_runtime
                    .pending()
                    .get(&instance_id)
                    .ok_or(CommandError::UnknownEventInstance(instance_id))?;
                if pending.recipient != player_id {
                    return Err(CommandError::WrongEventRecipient {
                        instance_id,
                        expected: pending.recipient,
                        actual: player_id,
                    });
                }
                state.event_runtime.close(instance_id);
                Ok(())
            }
        }
    }

    pub(crate) fn rebuild_derived_indexes(&mut self) -> Result<(), LoadValidationError> {
        self.state
            .map
            .rebuild_indexes()
            .map_err(|e| LoadValidationError::State(StateValidationError::Graph(e)))?;
        self.state
            .modifiers
            .rebuild_index()
            .map_err(|e| LoadValidationError::State(StateValidationError::Modifiers(e)))?;
        Ok(())
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
            match &command.action {
                CommandAction::AddCountry { tag, .. } => {
                    CountryTag::new(tag.as_str())
                        .map_err(|_| LoadValidationError::InvalidQueuedCountryTag)?;
                }
                CommandAction::SetStat { stat, .. } => {
                    StatId::new(stat.as_str())
                        .map_err(|_| LoadValidationError::InvalidQueuedStatId)?;
                }
                CommandAction::AddEntityTag { tag, .. }
                | CommandAction::RemoveEntityTag { tag, .. } => {
                    crate::entities::validate_key(tag).map_err(|error| {
                        LoadValidationError::InvalidQueuedEntityData(format!("{error:?}"))
                    })?;
                }
                CommandAction::SetComponentField {
                    component, field, ..
                }
                | CommandAction::RemoveComponentField {
                    component, field, ..
                } => {
                    crate::entities::validate_key(component).map_err(|error| {
                        LoadValidationError::InvalidQueuedEntityData(format!("{error:?}"))
                    })?;
                    crate::entities::validate_key(field).map_err(|error| {
                        LoadValidationError::InvalidQueuedEntityData(format!("{error:?}"))
                    })?;
                }
                CommandAction::AddModifier { modifier } => {
                    let mut candidate = ModifierSet::default();
                    candidate.add(modifier.clone()).map_err(|error| {
                        LoadValidationError::InvalidQueuedModifier(format!("{error:?}"))
                    })?;
                }
                CommandAction::OpenEvent {
                    event_id,
                    once,
                    cooldown_ticks,
                    ..
                } => {
                    crate::EventId::new(event_id.as_str())
                        .map_err(|_| LoadValidationError::InvalidQueuedEventId)?;
                    if !once && cooldown_ticks.is_none_or(|cooldown| cooldown == 0) {
                        return Err(LoadValidationError::InvalidQueuedEventCooldown);
                    }
                }
                CommandAction::ResolveEvent { instance_id } => {
                    let Some(pending) = self.state.event_runtime.pending().get(instance_id) else {
                        return Err(LoadValidationError::InvalidQueuedEventResolution);
                    };
                    if pending.recipient != command.player_id {
                        return Err(LoadValidationError::InvalidQueuedEventResolution);
                    }
                }
                _ => {}
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
    InvalidQueuedStatId,
    InvalidQueuedModifier(String),
    InvalidQueuedEventId,
    InvalidQueuedEventCooldown,
    InvalidQueuedEventResolution,
    InvalidQueuedEntityData(String),
}
