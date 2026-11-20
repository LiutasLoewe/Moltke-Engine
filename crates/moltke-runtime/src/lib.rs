//! Deterministic host for game-authored Rust simulation systems.
//!
//! Systems read one immutable world snapshot and return typed commands for the
//! next tick. The host executes systems in the validated content schedule,
//! gives each one a reproducible random stream, then queues all commands as a
//! single batch before advancing the simulation.

use moltke_content::{ContentError, SystemRegistry};
use moltke_core::{
    CanonicalStateHash, CommandAction, CommandOutcome, DeterministicRng, PlayerId, QueuedCommand,
    QueueError, Simulation, Tick, WorldState,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const SYSTEM_PLAYER: PlayerId = PlayerId(0);
const SYSTEM_SEQUENCE_BASE: u64 = 1 << 63;
const COMMANDS_PER_SYSTEM_LIMIT: usize = 4_096;
const COMMANDS_PER_TICK_LIMIT: usize = 65_536;
const COMMAND_SEQUENCE_STRIDE: u64 = 1 << 32;

/// A trusted, compiled game system. Implementations should be pure functions
/// of the supplied snapshot and context; authoritative mutations go through
/// the returned command list.
pub trait RuntimeSystem: Send + Sync {
    fn id(&self) -> &str;

    fn plan(
        &self,
        state: &WorldState,
        context: &mut SystemContext,
    ) -> Result<Vec<CommandAction>, String>;
}

/// Per-system tick information and a random stream derived from the previous
/// canonical state, next tick, and system ID.
pub struct SystemContext {
    current_tick: Tick,
    next_tick: Tick,
    rng: DeterministicRng,
}

impl SystemContext {
    pub fn current_tick(&self) -> Tick {
        self.current_tick
    }

    pub fn next_tick(&self) -> Tick {
        self.next_tick
    }

    pub fn rng(&mut self) -> &mut DeterministicRng {
        &mut self.rng
    }
}

/// Binds the data-defined system schedule to compiled Rust handlers.
pub struct RuntimeHost {
    schedule: Vec<String>,
    handlers: BTreeMap<String, Box<dyn RuntimeSystem>>,
}

impl RuntimeHost {
    pub fn new(
        registry: &SystemRegistry,
        enabled: Option<&BTreeSet<String>>,
        systems: impl IntoIterator<Item = Box<dyn RuntimeSystem>>,
    ) -> Result<Self, RuntimeHostError> {
        let schedule: Vec<String> = registry
            .enabled_schedule(enabled)
            .map_err(RuntimeHostError::Content)?
            .into_iter()
            .map(|definition| definition.id.clone())
            .collect();
        let known_ids: BTreeSet<&str> = registry
            .schedule()
            .iter()
            .map(|definition| definition.id.as_str())
            .collect();

        let mut handlers = BTreeMap::new();
        for system in systems {
            let id = system.id().to_owned();
            if !known_ids.contains(id.as_str()) {
                return Err(RuntimeHostError::UnknownHandler(id));
            }
            if handlers.insert(id.clone(), system).is_some() {
                return Err(RuntimeHostError::DuplicateHandler(id));
            }
        }
        for id in &schedule {
            if !handlers.contains_key(id) {
                return Err(RuntimeHostError::MissingHandler(id.clone()));
            }
        }
        Ok(Self { schedule, handlers })
    }

    pub fn schedule(&self) -> &[String] {
        &self.schedule
    }

    /// Plans every enabled system against the same state snapshot. If planning
    /// or command validation fails, no command is queued and the tick is not
    /// advanced. Command application errors are reported in the normal tick
    /// outcomes, just like player commands.
    pub fn step(
        &self,
        simulation: &mut Simulation,
    ) -> Result<Vec<CommandOutcome>, RuntimeHostError> {
        let current_tick = simulation.state().tick();
        let next_tick = Tick(
            current_tick
                .0
                .checked_add(1)
                .ok_or(RuntimeHostError::TickOverflow)?,
        );
        let state_hash = simulation
            .canonical_state_hash()
            .map_err(RuntimeHostError::StateHash)?;
        let mut commands = Vec::new();

        for (system_index, id) in self.schedule.iter().enumerate() {
            let system = self
                .handlers
                .get(id)
                .expect("enabled systems were validated at host creation");
            let mut context = SystemContext {
                current_tick,
                next_tick,
                rng: DeterministicRng::seeded(system_seed(state_hash, next_tick, id)),
            };
            let actions = system
                .plan(simulation.state(), &mut context)
                .map_err(|message| RuntimeHostError::Planning {
                    system_id: id.clone(),
                    message,
                })?;
            if actions.len() > COMMANDS_PER_SYSTEM_LIMIT {
                return Err(RuntimeHostError::TooManySystemCommands {
                    system_id: id.clone(),
                    count: actions.len(),
                    limit: COMMANDS_PER_SYSTEM_LIMIT,
                });
            }
            if commands.len().saturating_add(actions.len()) > COMMANDS_PER_TICK_LIMIT {
                return Err(RuntimeHostError::TooManyTickCommands {
                    limit: COMMANDS_PER_TICK_LIMIT,
                });
            }
            for (command_index, action) in actions.into_iter().enumerate() {
                let sequence = system_sequence(system_index, command_index)?;
                commands.push(QueuedCommand::new(
                    next_tick,
                    SYSTEM_PLAYER,
                    sequence,
                    action,
                ));
            }
        }

        simulation
            .schedule_commands(commands)
            .map_err(RuntimeHostError::Queue)?;
        simulation.step().map_err(RuntimeHostError::Clock)
    }
}

fn system_seed(state_hash: CanonicalStateHash, tick: Tick, system_id: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in state_hash
        .0
        .to_le_bytes()
        .into_iter()
        .chain(tick.0.to_le_bytes())
        .chain(system_id.bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn system_sequence(system_index: usize, command_index: usize) -> Result<u64, RuntimeHostError> {
    let system_index = u64::try_from(system_index).map_err(|_| RuntimeHostError::SequenceOverflow)?;
    let command_index = u64::try_from(command_index).map_err(|_| RuntimeHostError::SequenceOverflow)?;
    if command_index >= COMMAND_SEQUENCE_STRIDE {
        return Err(RuntimeHostError::SequenceOverflow);
    }
    SYSTEM_SEQUENCE_BASE
        .checked_add(
            system_index
                .checked_mul(COMMAND_SEQUENCE_STRIDE)
                .ok_or(RuntimeHostError::SequenceOverflow)?,
        )
        .and_then(|base| base.checked_add(command_index))
        .ok_or(RuntimeHostError::SequenceOverflow)
}

#[derive(Debug)]
pub enum RuntimeHostError {
    Content(ContentError),
    UnknownHandler(String),
    DuplicateHandler(String),
    MissingHandler(String),
    Planning {
        system_id: String,
        message: String,
    },
    TooManySystemCommands {
        system_id: String,
        count: usize,
        limit: usize,
    },
    TooManyTickCommands {
        limit: usize,
    },
    SequenceOverflow,
    TickOverflow,
    StateHash(serde_json::Error),
    Queue(QueueError),
    Clock(moltke_core::ClockError),
}

impl fmt::Display for RuntimeHostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "simulation runtime failed: {self:?}")
    }
}

impl std::error::Error for RuntimeHostError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Content(error) => Some(error),
            Self::StateHash(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moltke_content::{SystemDefinition, SystemPhase};
    use moltke_core::{EntityId, Fixed, StatId};

    struct RandomStatSystem {
        id: &'static str,
        stat: StatId,
    }

    impl RuntimeSystem for RandomStatSystem {
        fn id(&self) -> &str {
            self.id
        }

        fn plan(
            &self,
            _state: &WorldState,
            context: &mut SystemContext,
        ) -> Result<Vec<CommandAction>, String> {
            let raw = i64::from_ne_bytes(context.rng().next_u64().to_ne_bytes());
            Ok(vec![CommandAction::SetStat {
                entity: EntityId(12),
                stat: self.stat.clone(),
                value: Fixed::from_raw(raw),
            }])
        }
    }

    fn registry() -> SystemRegistry {
        SystemRegistry::new(vec![SystemDefinition {
            id: "world.random_stat".to_owned(),
            phase: SystemPhase::Gameplay,
            enabled_by_default: true,
            after: Vec::new(),
        }])
        .expect("valid registry")
    }

    fn host(registry: &SystemRegistry) -> RuntimeHost {
        RuntimeHost::new(
            registry,
            None,
            vec![Box::new(RandomStatSystem {
                id: "world.random_stat",
                stat: StatId::new("test_roll").expect("valid stat ID"),
            })],
        )
        .expect("valid runtime host")
    }

    #[test]
    fn system_randomness_and_results_repeat_from_identical_initial_state() {
        let registry = registry();
        let host = host(&registry);
        let mut first = Simulation::new(42);
        let mut second = Simulation::new(42);

        assert_eq!(
            host.step(&mut first).expect("first tick"),
            host.step(&mut second).expect("second tick")
        );
        assert_eq!(
            first.canonical_state_hash().expect("first hash"),
            second.canonical_state_hash().expect("second hash")
        );
        assert_eq!(first.state().tick(), Tick(1));
    }

    #[test]
    fn planning_failure_does_not_advance_or_queue_commands() {
        struct FailingSystem;
        impl RuntimeSystem for FailingSystem {
            fn id(&self) -> &str {
                "world.random_stat"
            }

            fn plan(
                &self,
                _state: &WorldState,
                _context: &mut SystemContext,
            ) -> Result<Vec<CommandAction>, String> {
                Err("intentional planning failure".to_owned())
            }
        }

        let registry = registry();
        let host = RuntimeHost::new(&registry, None, vec![Box::new(FailingSystem)])
            .expect("valid runtime host");
        let mut simulation = Simulation::new(42);

        assert!(matches!(
            host.step(&mut simulation),
            Err(RuntimeHostError::Planning { .. })
        ));
        assert_eq!(simulation.state().tick(), Tick(0));
        assert!(simulation.queued_commands().is_empty());
    }
}
