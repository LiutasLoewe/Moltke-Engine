//! Deterministic command replays and state checksums.

use crate::{ClockError, QueueError, QueuedCommand, SaveError, Simulation, Tick};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

const REPLAY_FORMAT_VERSION: u32 = 1;

/// Stable diagnostic checksum over the canonical serialized simulation state.
/// FNV-1a is intentionally non-cryptographic; it detects replay divergence,
/// not malicious tampering.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CanonicalStateHash(pub u64);

impl fmt::Display for CanonicalStateHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:016x}", self.0)
    }
}

impl Simulation {
    pub fn canonical_state_hash(&self) -> Result<CanonicalStateHash, serde_json::Error> {
        // The command queue is an input transport buffer. Hash authoritative
        // world state only so replay checkpoints do not depend on when future
        // commands were submitted to that buffer.
        let bytes = serde_json::to_vec(&self.state)?;
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in bytes {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Ok(CanonicalStateHash(hash))
    }
}

/// A single-player input log. The initial save contains the full starting
/// state; commands are stored separately and sorted by tick/player/sequence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Replay {
    format_version: u32,
    initial_tick: Tick,
    initial_save_json: String,
    commands: Vec<QueuedCommand>,
    checkpoints: BTreeMap<Tick, CanonicalStateHash>,
}

impl Replay {
    pub fn new(initial: &Simulation) -> Result<Self, ReplayError> {
        if !initial.queued_commands().is_empty() {
            return Err(ReplayError::InitialStateHasQueuedCommands);
        }
        Ok(Self {
            format_version: REPLAY_FORMAT_VERSION,
            initial_tick: initial.state().tick(),
            initial_save_json: initial.to_save_json().map_err(ReplayError::Save)?,
            commands: Vec::new(),
            checkpoints: BTreeMap::new(),
        })
    }

    pub fn initial_tick(&self) -> Tick {
        self.initial_tick
    }

    pub fn commands(&self) -> &[QueuedCommand] {
        &self.commands
    }

    pub fn checkpoints(&self) -> &BTreeMap<Tick, CanonicalStateHash> {
        &self.checkpoints
    }

    pub fn record_command(&mut self, command: QueuedCommand) -> Result<(), ReplayError> {
        if command.tick <= self.initial_tick {
            return Err(ReplayError::CommandNotAfterStart(command.tick));
        }
        let key = (command.tick, command.player_id, command.sequence);
        match self.commands.binary_search_by_key(&key, |queued| {
            (queued.tick, queued.player_id, queued.sequence)
        }) {
            Ok(_) => Err(ReplayError::DuplicateCommand {
                tick: command.tick,
                player_id: command.player_id,
                sequence: command.sequence,
            }),
            Err(index) => {
                self.commands.insert(index, command);
                Ok(())
            }
        }
    }

    pub fn record_checkpoint(&mut self, simulation: &Simulation) -> Result<(), ReplayError> {
        if simulation.state().tick() < self.initial_tick {
            return Err(ReplayError::CheckpointBeforeStart(
                simulation.state().tick(),
            ));
        }
        self.checkpoints.insert(
            simulation.state().tick(),
            simulation
                .canonical_state_hash()
                .map_err(ReplayError::Hash)?,
        );
        Ok(())
    }

    pub fn to_json(&self) -> Result<String, ReplayError> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(ReplayError::Json)
    }

    pub fn from_json(source: &str) -> Result<Self, ReplayError> {
        let replay: Self = serde_json::from_str(source).map_err(ReplayError::Json)?;
        replay.validate()?;
        Ok(replay)
    }

    /// Rebuilds and advances a replay through `target_tick`, checking every
    /// recorded checkpoint reached along the way.
    pub fn verify_until(&self, target_tick: Tick) -> Result<ReplayReport, ReplayError> {
        self.validate()?;
        if target_tick < self.initial_tick {
            return Err(ReplayError::TargetBeforeStart(target_tick));
        }

        let mut simulation =
            Simulation::from_save_json(&self.initial_save_json).map_err(ReplayError::Save)?;
        for command in self
            .commands
            .iter()
            .filter(|command| command.tick <= target_tick)
        {
            simulation
                .schedule_command(command.clone())
                .map_err(ReplayError::Queue)?;
        }

        if let Some(expected) = self.checkpoints.get(&self.initial_tick) {
            let actual = simulation
                .canonical_state_hash()
                .map_err(ReplayError::Hash)?;
            if actual != *expected {
                return Err(ReplayError::StateHashMismatch {
                    tick: self.initial_tick,
                    expected: *expected,
                    actual,
                });
            }
        }

        while simulation.state().tick() < target_tick {
            simulation.step().map_err(ReplayError::Clock)?;
            let tick = simulation.state().tick();
            if let Some(expected) = self.checkpoints.get(&tick) {
                let actual = simulation
                    .canonical_state_hash()
                    .map_err(ReplayError::Hash)?;
                if actual != *expected {
                    return Err(ReplayError::StateHashMismatch {
                        tick,
                        expected: *expected,
                        actual,
                    });
                }
            }
        }

        Ok(ReplayReport {
            final_tick: simulation.state().tick(),
            final_hash: simulation
                .canonical_state_hash()
                .map_err(ReplayError::Hash)?,
        })
    }

    fn validate(&self) -> Result<(), ReplayError> {
        if self.format_version != REPLAY_FORMAT_VERSION {
            return Err(ReplayError::UnsupportedFormat(self.format_version));
        }
        let initial =
            Simulation::from_save_json(&self.initial_save_json).map_err(ReplayError::Save)?;
        if initial.state().tick() != self.initial_tick || !initial.queued_commands().is_empty() {
            return Err(ReplayError::InvalidInitialState);
        }
        let mut previous = None;
        for command in &self.commands {
            let key = (command.tick, command.player_id, command.sequence);
            if command.tick <= self.initial_tick || previous.is_some_and(|last| last >= key) {
                return Err(ReplayError::InvalidCommandOrder);
            }
            previous = Some(key);
        }
        if self
            .checkpoints
            .keys()
            .any(|tick| *tick < self.initial_tick)
        {
            return Err(ReplayError::InvalidCheckpoint);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayReport {
    pub final_tick: Tick,
    pub final_hash: CanonicalStateHash,
}

#[derive(Debug)]
pub enum ReplayError {
    Json(serde_json::Error),
    Hash(serde_json::Error),
    Save(SaveError),
    Queue(QueueError),
    Clock(ClockError),
    UnsupportedFormat(u32),
    InitialStateHasQueuedCommands,
    InvalidInitialState,
    CommandNotAfterStart(Tick),
    DuplicateCommand {
        tick: Tick,
        player_id: crate::PlayerId,
        sequence: u64,
    },
    InvalidCommandOrder,
    CheckpointBeforeStart(Tick),
    InvalidCheckpoint,
    TargetBeforeStart(Tick),
    StateHashMismatch {
        tick: Tick,
        expected: CanonicalStateHash,
        actual: CanonicalStateHash,
    },
}

impl fmt::Display for ReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) | Self::Hash(error) => {
                write!(formatter, "replay JSON error: {error}")
            }
            Self::Save(error) => write!(formatter, "replay save error: {error}"),
            Self::Queue(error) => write!(formatter, "replay command error: {error:?}"),
            Self::Clock(error) => write!(formatter, "replay clock error: {error:?}"),
            Self::UnsupportedFormat(version) => {
                write!(formatter, "unsupported replay format {version}")
            }
            Self::InitialStateHasQueuedCommands => {
                formatter.write_str("replay start state must not contain queued commands")
            }
            Self::InvalidInitialState => formatter.write_str(
                "replay initial save does not match its declared tick or contains pending commands",
            ),
            Self::CommandNotAfterStart(tick) => write!(
                formatter,
                "replay command at tick {} is not after its initial tick",
                tick.0
            ),
            Self::DuplicateCommand {
                tick,
                player_id,
                sequence,
            } => write!(
                formatter,
                "duplicate replay command ({}, {}, {})",
                tick.0, player_id.0, sequence
            ),
            Self::InvalidCommandOrder => {
                formatter.write_str("replay commands are not in canonical order")
            }
            Self::CheckpointBeforeStart(tick) => write!(
                formatter,
                "checkpoint at tick {} precedes replay start",
                tick.0
            ),
            Self::InvalidCheckpoint => {
                formatter.write_str("replay contains a checkpoint before its start tick")
            }
            Self::TargetBeforeStart(tick) => write!(
                formatter,
                "requested replay tick {} precedes replay start",
                tick.0
            ),
            Self::StateHashMismatch {
                tick,
                expected,
                actual,
            } => write!(
                formatter,
                "replay diverged at tick {}: expected {expected}, got {actual}",
                tick.0
            ),
        }
    }
}

impl std::error::Error for ReplayError {}
