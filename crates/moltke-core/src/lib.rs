//! Renderer-independent simulation foundations for Moltke Engine.
//!
//! State changes enter through commands scheduled for a future integer tick.
//! Collections and command ordering are stable so callers can build repeatable
//! replays and, later, a deterministic multiplayer protocol.

mod calendar;
mod clock;
mod command;
mod events;
mod entities;
mod ids;
mod map_graph;
mod model;
mod modifiers;
mod replay;
mod rng;
mod save;
mod simulation;
mod trigger;

pub use calendar::{CalendarConfig, CalendarDate, CalendarError};
pub use clock::{FixedStepClock, PacerError};
pub use command::{CommandAction, CommandError, CommandOutcome, QueuedCommand};
pub use events::{EventId, EventInstanceId, EventRuntimeState, EventStateError, PendingEvent};
pub use entities::{ComponentValue, EntityStore, EntityStoreError, GenericEntity};
pub use ids::{CountryId, CountryTag, InvalidCountryTag, PlayerId, ProvinceId, Tick};
pub use map_graph::{EdgeId, GraphError, MapEdge, MapGraph, MapNode, NodeId, Path, Position3};
pub use model::{Country, Province, StateValidationError, WorldState};
pub use modifiers::{
    EntityId, FIXED_SCALE, Fixed, Modifier, ModifierError, ModifierId, ModifierKind, ModifierSet,
    StatId,
};
pub use replay::{CanonicalStateHash, Replay, ReplayError, ReplayReport};
pub use rng::DeterministicRng;
pub use save::SaveError;
pub use simulation::{ClockError, QueueError, Simulation};
pub use trigger::{Comparison, TriggerError, TriggerExpr};
