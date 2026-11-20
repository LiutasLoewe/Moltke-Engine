//! Serializable state for data-defined events. Event rules live in game
//! content; pending player choices and cooldowns belong to the save state.

use crate::{PlayerId, Tick};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventId(String);

impl EventId {
    pub fn new(value: impl Into<String>) -> Result<Self, EventStateError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 96
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'_' | b'-' | b'.')
            })
            && value.as_bytes()[0].is_ascii_lowercase();
        if valid {
            Ok(Self(value))
        } else {
            Err(EventStateError::InvalidEventId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventInstanceId(pub u64);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PendingEvent {
    pub instance_id: EventInstanceId,
    pub event_id: EventId,
    pub recipient: PlayerId,
    pub triggered_at: Tick,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventRuntimeState {
    #[serde(default = "first_instance_id")]
    next_instance_id: u64,
    pending: BTreeMap<EventInstanceId, PendingEvent>,
    last_triggered: BTreeMap<EventId, Tick>,
    fired_once: BTreeSet<EventId>,
}

impl Default for EventRuntimeState {
    fn default() -> Self {
        Self {
            next_instance_id: first_instance_id(),
            pending: BTreeMap::new(),
            last_triggered: BTreeMap::new(),
            fired_once: BTreeSet::new(),
        }
    }
}

impl EventRuntimeState {
    pub fn pending(&self) -> &BTreeMap<EventInstanceId, PendingEvent> {
        &self.pending
    }

    pub fn can_fire(
        &self,
        id: &EventId,
        now: Tick,
        once: bool,
        cooldown_ticks: Option<u64>,
    ) -> bool {
        if self.pending.values().any(|event| &event.event_id == id) {
            return false;
        }
        if once && self.fired_once.contains(id) {
            return false;
        }
        match (self.last_triggered.get(id), cooldown_ticks) {
            (Some(last), Some(cooldown)) => now.0.saturating_sub(last.0) >= cooldown,
            (Some(_), None) if !once => false,
            _ => true,
        }
    }

    pub fn open(
        &mut self,
        id: EventId,
        recipient: PlayerId,
        now: Tick,
        once: bool,
    ) -> Result<PendingEvent, EventStateError> {
        let instance_id = EventInstanceId(self.next_instance_id.max(1));
        self.next_instance_id = instance_id
            .0
            .checked_add(1)
            .ok_or(EventStateError::InstanceIdOverflow)?;
        let pending = PendingEvent {
            instance_id,
            event_id: id.clone(),
            recipient,
            triggered_at: now,
        };
        self.last_triggered.insert(id.clone(), now);
        if once {
            self.fired_once.insert(id);
        }
        self.pending.insert(instance_id, pending.clone());
        Ok(pending)
    }

    pub fn close(&mut self, instance_id: EventInstanceId) -> Option<PendingEvent> {
        self.pending.remove(&instance_id)
    }

    pub(crate) fn validate(&self, current_tick: Tick) -> Result<(), EventStateError> {
        if self.next_instance_id == 0 {
            return Err(EventStateError::CorruptInstanceSequence);
        }
        for (id, event) in &self.pending {
            EventId::new(event.event_id.as_str())?;
            if *id != event.instance_id
                || id.0 >= self.next_instance_id
                || event.triggered_at > current_tick
            {
                return Err(EventStateError::CorruptPendingEvent(*id));
            }
        }
        for id in self.last_triggered.keys().chain(self.fired_once.iter()) {
            EventId::new(id.as_str())?;
        }
        if self
            .last_triggered
            .values()
            .any(|tick| *tick > current_tick)
        {
            return Err(EventStateError::FutureTriggerTick);
        }
        Ok(())
    }
}

fn first_instance_id() -> u64 {
    1
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventStateError {
    InvalidEventId(String),
    InstanceIdOverflow,
    CorruptInstanceSequence,
    CorruptPendingEvent(EventInstanceId),
    FutureTriggerTick,
}

impl fmt::Display for EventStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid event state: {self:?}")
    }
}

impl std::error::Error for EventStateError {}
