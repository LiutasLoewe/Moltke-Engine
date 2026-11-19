//! Renderer-independent simulation foundations for Moltke Engine.
//!
//! State changes enter through commands scheduled for a future integer tick.
//! Collections and command ordering are stable so callers can build repeatable
//! replays and, later, a deterministic multiplayer protocol.

mod command;
mod map_graph;
mod modifiers;
mod ids;
mod model;
mod rng;
mod save;
mod simulation;

pub use command::{CommandAction, CommandError, CommandOutcome, QueuedCommand};
pub use map_graph::{EdgeId, GraphError, MapEdge, MapGraph, MapNode, NodeId, Path, Position3};
pub use modifiers::{EntityId, Fixed, Modifier, ModifierError, ModifierId, ModifierKind, ModifierSet, StatId, FIXED_SCALE};
pub use ids::{CountryId, CountryTag, InvalidCountryTag, PlayerId, ProvinceId, Tick};
pub use model::{Country, Province, StateValidationError, WorldState};
pub use rng::DeterministicRng;
pub use save::SaveError;
pub use simulation::{ClockError, QueueError, Simulation};
