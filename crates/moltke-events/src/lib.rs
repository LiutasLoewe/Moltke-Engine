//! Data-driven triggers, events, and player choices.
//!
//! Event definitions are immutable game content. Pending choices, cooldowns,
//! and once-only completion state live in `moltke-core` and therefore travel
//! with normal simulation saves.

use moltke_content::{ContentError, parse_toml};
use moltke_core::{
    CommandAction, EventId, EventInstanceId, EventStateError, PlayerId, QueueError, QueuedCommand,
    Simulation, Tick, TriggerError, TriggerExpr,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventOption {
    pub id: String,
    pub localization_key: String,
    #[serde(default)]
    pub effects: Vec<CommandAction>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventDefinition {
    pub id: EventId,
    pub recipient: PlayerId,
    pub trigger: TriggerExpr,
    #[serde(default = "default_once")]
    pub once: bool,
    #[serde(default)]
    pub cooldown_ticks: Option<u64>,
    /// Optional geometric hazard interval. A value of N gives the event a
    /// deterministic seeded probability of 1/N on each eligible tick.
    #[serde(default)]
    pub mean_time_to_happen_ticks: Option<u64>,
    pub options: Vec<EventOption>,
}

fn default_once() -> bool {
    true
}

impl EventDefinition {
    fn validate(&self) -> Result<(), EventCatalogError> {
        EventId::new(self.id.as_str()).map_err(EventCatalogError::InvalidEventId)?;
        self.trigger
            .validate()
            .map_err(EventCatalogError::Trigger)?;
        if self.options.is_empty() {
            return Err(EventCatalogError::NoOptions(self.id.clone()));
        }
        if self.options.len() > 4_095 {
            return Err(EventCatalogError::TooManyOptions(self.id.clone()));
        }
        if !self.once && self.cooldown_ticks.is_none_or(|cooldown| cooldown == 0) {
            return Err(EventCatalogError::RepeatNeedsCooldown(self.id.clone()));
        }
        if self.mean_time_to_happen_ticks == Some(0) {
            return Err(EventCatalogError::InvalidMeanTimeToHappen(self.id.clone()));
        }
        let mut ids = std::collections::BTreeSet::new();
        for option in &self.options {
            if !valid_option_id(&option.id) || !ids.insert(option.id.as_str()) {
                return Err(EventCatalogError::InvalidOptionId {
                    event_id: self.id.clone(),
                    option_id: option.id.clone(),
                });
            }
            if option.localization_key.trim().is_empty() {
                return Err(EventCatalogError::EmptyLocalizationKey(self.id.clone()));
            }
        }
        Ok(())
    }
}

fn valid_option_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value.as_bytes()[0].is_ascii_lowercase()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EventCatalog {
    definitions: BTreeMap<EventId, EventDefinition>,
}

impl EventCatalog {
    pub fn insert(&mut self, definition: EventDefinition) -> Result<(), EventCatalogError> {
        definition.validate()?;
        if self.definitions.contains_key(&definition.id) {
            return Err(EventCatalogError::DuplicateEvent(definition.id));
        }
        self.definitions.insert(definition.id.clone(), definition);
        Ok(())
    }

    pub fn get(&self, id: &EventId) -> Option<&EventDefinition> {
        self.definitions.get(id)
    }

    pub fn definitions(&self) -> &BTreeMap<EventId, EventDefinition> {
        &self.definitions
    }

    pub fn from_toml(path: &str, source: &str) -> Result<Self, EventLoadError> {
        let file: EventFile = parse_toml(path, source).map_err(EventLoadError::Content)?;
        let mut catalog = Self::default();
        for definition in file.events {
            catalog
                .insert(definition)
                .map_err(EventLoadError::Catalog)?;
        }
        Ok(catalog)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EventFile {
    #[serde(default)]
    events: Vec<EventDefinition>,
}

/// Runs event triggers at a simulation boundary and turns player choices into
/// atomically queued simulation commands.
#[derive(Clone, Copy, Debug, Default)]
pub struct EventEngine;

impl EventEngine {
    pub fn evaluate(
        &self,
        simulation: &mut Simulation,
        catalog: &EventCatalog,
    ) -> Result<Vec<QueuedCommand>, EventEngineError> {
        let now = simulation.state().tick();
        let target_tick = Tick(now.0.checked_add(1).ok_or(EventEngineError::TickOverflow)?);
        // Player zero is reserved for deterministic engine-generated commands.
        let system_player = PlayerId(0);
        let mut used_sequences: BTreeSet<u64> = simulation
            .queued_commands()
            .iter()
            .filter(|command| command.tick == target_tick && command.player_id == system_player)
            .map(|command| command.sequence)
            .collect();
        let already_queued: BTreeSet<EventId> = simulation
            .queued_commands()
            .iter()
            .filter_map(|command| {
                if command.tick != target_tick {
                    return None;
                }
                match &command.action {
                    CommandAction::OpenEvent { event_id, .. } => Some(event_id.clone()),
                    _ => None,
                }
            })
            .collect();
        let mut next_sequence = 0_u64;
        let mut commands = Vec::new();
        for definition in catalog.definitions.values() {
            if already_queued.contains(&definition.id) {
                continue;
            }
            let can_fire = simulation.state().event_runtime().can_fire(
                &definition.id,
                now,
                definition.once,
                definition.cooldown_ticks,
            );
            if !can_fire {
                continue;
            }
            let triggered = definition
                .trigger
                .evaluate(simulation.state_mut())
                .map_err(EventEngineError::Trigger)?;
            if triggered
                && definition
                    .mean_time_to_happen_ticks
                    .is_none_or(|mean| mtth_roll_passes(simulation, mean))
            {
                while used_sequences.contains(&next_sequence) {
                    next_sequence = next_sequence
                        .checked_add(1)
                        .ok_or(EventEngineError::SequenceOverflow)?;
                }
                let sequence = next_sequence;
                used_sequences.insert(sequence);
                commands.push(QueuedCommand::new(
                    target_tick,
                    system_player,
                    sequence,
                    CommandAction::OpenEvent {
                        event_id: definition.id.clone(),
                        recipient: definition.recipient,
                        once: definition.once,
                        cooldown_ticks: definition.cooldown_ticks,
                    },
                ));
            }
        }
        simulation
            .schedule_commands(commands.clone())
            .map_err(EventEngineError::Queue)?;
        Ok(commands)
    }

    pub fn choose(
        &self,
        simulation: &mut Simulation,
        catalog: &EventCatalog,
        instance_id: EventInstanceId,
        player_id: PlayerId,
        option_id: &str,
        first_sequence: u64,
    ) -> Result<Vec<QueuedCommand>, EventEngineError> {
        let pending = simulation
            .state()
            .event_runtime()
            .pending()
            .get(&instance_id)
            .cloned()
            .ok_or(EventEngineError::UnknownInstance(instance_id))?;
        let already_submitted = simulation.queued_commands().iter().any(|command| {
            matches!(&command.action, CommandAction::ResolveEvent { instance_id: queued_id } if *queued_id == instance_id)
        });
        if already_submitted {
            return Err(EventEngineError::ChoiceAlreadySubmitted(instance_id));
        }
        if pending.recipient != player_id {
            return Err(EventEngineError::WrongRecipient {
                instance_id,
                expected: pending.recipient,
                actual: player_id,
            });
        }
        let definition = catalog
            .get(&pending.event_id)
            .ok_or_else(|| EventEngineError::MissingDefinition(pending.event_id.clone()))?;
        let option = definition
            .options
            .iter()
            .find(|option| option.id == option_id)
            .ok_or_else(|| EventEngineError::UnknownOption {
                event_id: pending.event_id.clone(),
                option_id: option_id.to_owned(),
            })?;

        let target_tick = Tick(
            simulation
                .state()
                .tick()
                .0
                .checked_add(1)
                .ok_or(EventEngineError::TickOverflow)?,
        );
        let mut commands = Vec::with_capacity(option.effects.len() + 1);
        commands.push(QueuedCommand::new(
            target_tick,
            player_id,
            first_sequence,
            CommandAction::ResolveEvent { instance_id },
        ));
        for (offset, action) in option.effects.iter().enumerate() {
            let sequence = first_sequence
                .checked_add(u64::try_from(offset).map_err(|_| EventEngineError::SequenceOverflow)?)
                .and_then(|sequence| sequence.checked_add(1))
                .ok_or(EventEngineError::SequenceOverflow)?;
            commands.push(QueuedCommand::new(
                target_tick,
                player_id,
                sequence,
                action.clone(),
            ));
        }

        simulation
            .schedule_commands(commands.clone())
            .map_err(EventEngineError::Queue)?;
        Ok(commands)
    }
}

fn mtth_roll_passes(simulation: &mut Simulation, mean_ticks: u64) -> bool {
    let rejection_threshold = mean_ticks.wrapping_neg() % mean_ticks;
    loop {
        let draw = simulation.state_mut().random_u64();
        if draw >= rejection_threshold {
            return draw % mean_ticks == 0;
        }
    }
}

#[derive(Debug)]
pub enum EventCatalogError {
    InvalidEventId(EventStateError),
    Trigger(TriggerError),
    DuplicateEvent(EventId),
    NoOptions(EventId),
    RepeatNeedsCooldown(EventId),
    InvalidMeanTimeToHappen(EventId),
    TooManyOptions(EventId),
    InvalidOptionId {
        event_id: EventId,
        option_id: String,
    },
    EmptyLocalizationKey(EventId),
}

#[derive(Debug)]
pub enum EventLoadError {
    Content(ContentError),
    Catalog(EventCatalogError),
}

#[derive(Debug)]
pub enum EventEngineError {
    Trigger(TriggerError),
    Queue(QueueError),
    UnknownInstance(EventInstanceId),
    MissingDefinition(EventId),
    WrongRecipient {
        instance_id: EventInstanceId,
        expected: PlayerId,
        actual: PlayerId,
    },
    UnknownOption {
        event_id: EventId,
        option_id: String,
    },
    ChoiceAlreadySubmitted(EventInstanceId),
    TickOverflow,
    SequenceOverflow,
}

impl fmt::Display for EventCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid event catalog: {self:?}")
    }
}

impl std::error::Error for EventCatalogError {}

impl fmt::Display for EventLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Content(error) => write!(formatter, "event content error: {error}"),
            Self::Catalog(error) => write!(formatter, "event definition error: {error}"),
        }
    }
}

impl std::error::Error for EventLoadError {}

impl fmt::Display for EventEngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trigger(error) => write!(formatter, "event trigger failed: {error:?}"),
            Self::Queue(error) => write!(formatter, "event effects could not be queued: {error:?}"),
            Self::UnknownInstance(id) => {
                write!(formatter, "unknown pending event instance {}", id.0)
            }
            Self::MissingDefinition(id) => {
                write!(formatter, "event definition {:?} is missing", id.as_str())
            }
            Self::WrongRecipient {
                instance_id,
                expected,
                actual,
            } => write!(
                formatter,
                "player {} cannot choose event instance {} assigned to player {}",
                actual.0, instance_id.0, expected.0
            ),
            Self::UnknownOption {
                event_id,
                option_id,
            } => write!(
                formatter,
                "event {:?} has no option {option_id:?}",
                event_id.as_str()
            ),
            Self::ChoiceAlreadySubmitted(id) => write!(
                formatter,
                "a choice is already queued for event instance {}",
                id.0
            ),
            Self::TickOverflow => {
                formatter.write_str("simulation tick overflow while queuing event effects")
            }
            Self::SequenceOverflow => {
                formatter.write_str("command sequence overflow while queuing event effects")
            }
        }
    }
}

impl std::error::Error for EventEngineError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn event_with_mtth(mean_time_to_happen_ticks: u64) -> EventDefinition {
        EventDefinition {
            id: EventId::new("test_event").expect("valid event id"),
            recipient: PlayerId(1),
            trigger: TriggerExpr::Always,
            once: true,
            cooldown_ticks: None,
            mean_time_to_happen_ticks: Some(mean_time_to_happen_ticks),
            options: vec![EventOption {
                id: "accept".to_owned(),
                localization_key: "event.test.accept".to_owned(),
                effects: Vec::new(),
            }],
        }
    }

    #[test]
    fn one_tick_mean_time_triggers_immediately() {
        let mut catalog = EventCatalog::default();
        catalog
            .insert(event_with_mtth(1))
            .expect("valid event definition");
        let commands = EventEngine
            .evaluate(&mut Simulation::new(11), &catalog)
            .expect("valid event evaluation");
        assert_eq!(commands.len(), 1);
    }

    #[test]
    fn zero_mean_time_is_rejected() {
        let mut catalog = EventCatalog::default();
        assert!(matches!(
            catalog.insert(event_with_mtth(0)),
            Err(EventCatalogError::InvalidMeanTimeToHappen(_))
        ));
    }
}
