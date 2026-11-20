//! Deterministic utility scoring. Candidate generation belongs to game rules;
//! this crate scores immutable candidates and emits stable command order.

use moltke_core::{CommandAction, EntityId, FIXED_SCALE, Fixed, PlayerId, QueuedCommand, Tick};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::thread;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilityCandidate {
    pub actor_id: EntityId,
    pub action_id: String,
    pub base_score: Fixed,
    pub features: BTreeMap<String, Fixed>,
    pub command: CommandAction,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UtilityWeights {
    pub by_feature: BTreeMap<String, Fixed>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoredCandidate {
    pub actor_id: EntityId,
    pub action_id: String,
    pub score: Fixed,
    pub command: CommandAction,
}

/// Scores candidates on independent workers, then sorts by actor ID, descending
/// score, and action ID. Worker completion order never affects the result.
pub fn rank_candidates(
    candidates: &[UtilityCandidate],
    weights: &UtilityWeights,
    worker_count: usize,
) -> Result<Vec<ScoredCandidate>, AiError> {
    if worker_count == 0 {
        return Err(AiError::ZeroWorkers);
    }
    let mut identities = BTreeSet::new();
    for candidate in candidates {
        if !valid_action_id(&candidate.action_id) {
            return Err(AiError::InvalidActionId(candidate.action_id.clone()));
        }
        if !identities.insert((candidate.actor_id, candidate.action_id.as_str())) {
            return Err(AiError::DuplicateAction {
                actor_id: candidate.actor_id,
                action_id: candidate.action_id.clone(),
            });
        }
    }
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    let workers = worker_count.min(candidates.len());
    let chunk_size = candidates.len().div_ceil(workers);
    let mut scored = Vec::with_capacity(candidates.len());
    thread::scope(|scope| -> Result<(), AiError> {
        let handles: Vec<_> = candidates
            .chunks(chunk_size)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|candidate| score_one(candidate, weights))
                        .collect::<Result<Vec<_>, _>>()
                })
            })
            .collect();
        for handle in handles {
            let chunk = handle.join().map_err(|_| AiError::WorkerPanicked)??;
            scored.extend(chunk);
        }
        Ok(())
    })?;

    scored.sort_by(|left, right| {
        left.actor_id
            .cmp(&right.actor_id)
            .then_with(|| right.score.cmp(&left.score))
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    Ok(scored)
}

/// Selects the highest utility option for each actor. Equal scores resolve to
/// the lexicographically smallest action ID.
pub fn best_per_actor(scored: &[ScoredCandidate]) -> Vec<&ScoredCandidate> {
    let mut best = Vec::new();
    let mut previous_actor = None;
    for candidate in scored {
        if previous_actor != Some(candidate.actor_id) {
            best.push(candidate);
            previous_actor = Some(candidate.actor_id);
        }
    }
    best
}

/// Converts one selected action per actor into commands for a future tick.
pub fn commands_for_tick(
    best: &[&ScoredCandidate],
    tick: Tick,
    player_id: PlayerId,
    first_sequence: u64,
) -> Result<Vec<QueuedCommand>, AiError> {
    let mut commands = Vec::with_capacity(best.len());
    for (offset, candidate) in best.iter().enumerate() {
        let sequence = first_sequence
            .checked_add(u64::try_from(offset).map_err(|_| AiError::SequenceOverflow)?)
            .ok_or(AiError::SequenceOverflow)?;
        commands.push(QueuedCommand::new(
            tick,
            player_id,
            sequence,
            candidate.command.clone(),
        ));
    }
    Ok(commands)
}

fn score_one(
    candidate: &UtilityCandidate,
    weights: &UtilityWeights,
) -> Result<ScoredCandidate, AiError> {
    let mut score = i128::from(candidate.base_score.raw());
    for (feature, value) in &candidate.features {
        if let Some(weight) = weights.by_feature.get(feature) {
            let contribution = i128::from(value.raw())
                .checked_mul(i128::from(weight.raw()))
                .ok_or(AiError::ArithmeticOverflow)?
                / i128::from(FIXED_SCALE);
            score = score
                .checked_add(contribution)
                .ok_or(AiError::ArithmeticOverflow)?;
        }
    }
    let score = i64::try_from(score).map_err(|_| AiError::ArithmeticOverflow)?;
    Ok(ScoredCandidate {
        actor_id: candidate.actor_id,
        action_id: candidate.action_id.clone(),
        score: Fixed::from_raw(score),
        command: candidate.command.clone(),
    })
}

fn valid_action_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value.as_bytes()[0].is_ascii_lowercase()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AiError {
    ZeroWorkers,
    InvalidActionId(String),
    DuplicateAction {
        actor_id: EntityId,
        action_id: String,
    },
    ArithmeticOverflow,
    SequenceOverflow,
    WorkerPanicked,
}

impl fmt::Display for AiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "AI planning failed: {self:?}")
    }
}

impl std::error::Error for AiError {}

#[cfg(test)]
mod tests {
    use super::*;
    use moltke_core::StatId;

    #[test]
    fn utility_results_do_not_depend_on_worker_count_or_input_completion_order() {
        let stability = StatId::new("stability").expect("valid stat ID");
        let candidates = vec![
            UtilityCandidate {
                actor_id: EntityId(2),
                action_id: "raise_tax".to_owned(),
                base_score: Fixed::from_raw(500),
                features: BTreeMap::from([("income".to_owned(), Fixed::from_raw(2_000))]),
                command: CommandAction::SetStat {
                    entity: EntityId(2),
                    stat: stability.clone(),
                    value: Fixed::from_raw(1_000),
                },
            },
            UtilityCandidate {
                actor_id: EntityId(1),
                action_id: "build".to_owned(),
                base_score: Fixed::from_raw(1_000),
                features: BTreeMap::from([("growth".to_owned(), Fixed::from_raw(3_000))]),
                command: CommandAction::SetStat {
                    entity: EntityId(1),
                    stat: stability.clone(),
                    value: Fixed::from_raw(2_000),
                },
            },
            UtilityCandidate {
                actor_id: EntityId(1),
                action_id: "research".to_owned(),
                base_score: Fixed::from_raw(1_500),
                features: BTreeMap::from([("growth".to_owned(), Fixed::from_raw(1_000))]),
                command: CommandAction::SetStat {
                    entity: EntityId(1),
                    stat: stability,
                    value: Fixed::from_raw(3_000),
                },
            },
        ];
        let weights = UtilityWeights {
            by_feature: BTreeMap::from([
                ("growth".to_owned(), Fixed::from_raw(2_000)),
                ("income".to_owned(), Fixed::from_raw(500)),
            ]),
        };

        let expected = rank_candidates(&candidates, &weights, 1).expect("single worker result");
        for workers in [2, 3, 8] {
            assert_eq!(
                rank_candidates(&candidates, &weights, workers).expect("parallel result"),
                expected
            );
        }
    }
}
