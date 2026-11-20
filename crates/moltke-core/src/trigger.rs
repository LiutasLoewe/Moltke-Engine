use crate::{EntityId, Fixed, ModifierError, StatId, Tick, WorldState};
use serde::{Deserialize, Serialize};

const MAX_TRIGGER_DEPTH: usize = 128;
const MAX_TRIGGER_NODES: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Comparison { Equal, NotEqual, Less, LessOrEqual, Greater, GreaterOrEqual }

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TriggerExpr {
    Always,
    TickAtLeast(Tick),
    StatCompare { entity: EntityId, stat: StatId, comparison: Comparison, value: Fixed },
    All(Vec<TriggerExpr>),
    Any(Vec<TriggerExpr>),
    Not(Box<TriggerExpr>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TriggerError {
    TooDeep,
    TooManyNodes,
    InvalidStatId(String),
    Modifier(ModifierError),
}

impl TriggerExpr {
    /// Evaluates against authoritative state. All/Any use deterministic
    /// short-circuit order and empty All/Any follow their usual identities.
    pub fn evaluate(&self, state: &mut WorldState) -> Result<bool, TriggerError> {
        self.validate()?;
        self.evaluate_inner(state)
    }

    pub fn validate(&self) -> Result<(), TriggerError> {
        fn visit(expr: &TriggerExpr, depth: usize, count: &mut usize) -> Result<(), TriggerError> {
            if depth > MAX_TRIGGER_DEPTH { return Err(TriggerError::TooDeep); }
            *count += 1;
            if *count > MAX_TRIGGER_NODES { return Err(TriggerError::TooManyNodes); }
            match expr {
                TriggerExpr::StatCompare { stat, .. } => {
                    StatId::new(stat.as_str()).map_err(|_| TriggerError::InvalidStatId(stat.as_str().to_owned()))?;
                }
                TriggerExpr::All(children) | TriggerExpr::Any(children) => {
                    for child in children { visit(child, depth + 1, count)?; }
                }
                TriggerExpr::Not(child) => visit(child, depth + 1, count)?,
                TriggerExpr::Always | TriggerExpr::TickAtLeast(_) => {}
            }
            Ok(())
        }
        let mut count = 0;
        visit(self, 0, &mut count)
    }

    fn evaluate_inner(&self, state: &mut WorldState) -> Result<bool, TriggerError> {
        match self {
            TriggerExpr::Always => Ok(true),
            TriggerExpr::TickAtLeast(tick) => Ok(state.tick() >= *tick),
            TriggerExpr::StatCompare { entity, stat, comparison, value } => {
                let now = state.tick();
                let actual = state.stat_value(*entity, stat, now)
                    .map_err(TriggerError::Modifier)?;
                Ok(match comparison {
                    Comparison::Equal => actual == *value,
                    Comparison::NotEqual => actual != *value,
                    Comparison::Less => actual < *value,
                    Comparison::LessOrEqual => actual <= *value,
                    Comparison::Greater => actual > *value,
                    Comparison::GreaterOrEqual => actual >= *value,
                })
            }
            TriggerExpr::All(children) => {
                for child in children {
                    if !child.evaluate_inner(state)? { return Ok(false); }
                }
                Ok(true)
            }
            TriggerExpr::Any(children) => {
                for child in children {
                    if child.evaluate_inner(state)? { return Ok(true); }
                }
                Ok(false)
            }
            TriggerExpr::Not(child) => Ok(!child.evaluate_inner(state)?),
        }
    }
}
