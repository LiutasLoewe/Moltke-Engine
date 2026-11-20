use crate::Tick;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const FIXED_SCALE: i64 = 10_000;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId(pub u64);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModifierId(pub u64);
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StatId(String);

impl StatId {
    pub fn new(value: impl Into<String>) -> Result<Self, ModifierError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 64
            && value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.');
        if valid {
            Ok(Self(value))
        } else {
            Err(ModifierError::InvalidStatId(value))
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Fixed(i64);
impl Fixed {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(FIXED_SCALE);
    pub const fn from_raw(raw: i64) -> Self {
        Self(raw)
    }
    pub const fn raw(self) -> i64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ModifierKind {
    Additive,
    Multiplicative,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Modifier {
    pub id: ModifierId,
    pub target: EntityId,
    pub stat: StatId,
    pub kind: ModifierKind,
    /// Add amount or multiplier factor; 10,000 is 1.0.
    pub value: Fixed,
    pub source: String,
    /// Active strictly before this tick; None means permanent.
    pub expires_at: Option<Tick>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModifierError {
    InvalidStatId(String),
    EmptySource,
    DuplicateId(ModifierId),
    UnknownId(ModifierId),
    InvalidMultiplier(ModifierId),
    ArithmeticOverflow,
    CorruptIndex,
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CacheKey {
    target: EntityId,
    stat: StatId,
    base: Fixed,
}

/// Ordered modifier store. Derived cache entries are excluded from saves.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModifierSet {
    modifiers: BTreeMap<ModifierId, Modifier>,
    #[serde(skip, default)]
    by_target_stat: BTreeMap<(EntityId, StatId), BTreeSet<ModifierId>>,
    #[serde(skip, default)]
    cache_tick: Option<Tick>,
    #[serde(skip, default)]
    cache: BTreeMap<CacheKey, Fixed>,
}

impl ModifierSet {
    pub fn add(&mut self, modifier: Modifier) -> Result<(), ModifierError> {
        if modifier.source.trim().is_empty() {
            return Err(ModifierError::EmptySource);
        }
        StatId::new(modifier.stat.as_str())?;
        if modifier.kind == ModifierKind::Multiplicative && modifier.value.raw() <= 0 {
            return Err(ModifierError::InvalidMultiplier(modifier.id));
        }
        if self.modifiers.contains_key(&modifier.id) {
            return Err(ModifierError::DuplicateId(modifier.id));
        }
        let id = modifier.id;
        let key = (modifier.target, modifier.stat.clone());
        self.modifiers.insert(id, modifier);
        self.by_target_stat.entry(key).or_default().insert(id);
        self.clear_cache();
        Ok(())
    }

    pub fn remove(&mut self, id: ModifierId) -> Result<Modifier, ModifierError> {
        let modifier = self
            .modifiers
            .get(&id)
            .ok_or(ModifierError::UnknownId(id))?;
        let key = (modifier.target, modifier.stat.clone());
        if !self
            .by_target_stat
            .get(&key)
            .is_some_and(|ids| ids.contains(&id))
        {
            return Err(ModifierError::CorruptIndex);
        }
        let removed = self
            .modifiers
            .remove(&id)
            .ok_or(ModifierError::UnknownId(id))?;
        let empty = {
            let ids = self
                .by_target_stat
                .get_mut(&key)
                .ok_or(ModifierError::CorruptIndex)?;
            ids.remove(&id);
            ids.is_empty()
        };
        if empty {
            self.by_target_stat.remove(&key);
        }
        self.clear_cache();
        Ok(removed)
    }

    pub fn remove_expired(&mut self, now: Tick) -> usize {
        let expired: Vec<_> = self
            .modifiers
            .iter()
            .filter(|(_, m)| m.expires_at.is_some_and(|end| end <= now))
            .map(|(id, _)| *id)
            .collect();
        for id in &expired {
            if let Some(modifier) = self.modifiers.remove(id) {
                let key = (modifier.target, modifier.stat);
                let empty = if let Some(ids) = self.by_target_stat.get_mut(&key) {
                    ids.remove(id);
                    ids.is_empty()
                } else {
                    false
                };
                if empty {
                    self.by_target_stat.remove(&key);
                }
            }
        }
        if !expired.is_empty() {
            self.clear_cache();
        }
        expired.len()
    }

    /// Additive values are summed first; factors apply in ModifierId order.
    /// Each multiplication truncates toward zero at FIXED_SCALE.
    pub fn evaluate(
        &mut self,
        target: EntityId,
        stat: &StatId,
        base: Fixed,
        now: Tick,
    ) -> Result<Fixed, ModifierError> {
        if self.cache_tick != Some(now) {
            self.cache.clear();
            self.cache_tick = Some(now);
        }
        let key = CacheKey {
            target,
            stat: stat.clone(),
            base,
        };
        if let Some(result) = self.cache.get(&key) {
            return Ok(*result);
        }

        let ids = self
            .by_target_stat
            .get(&(target, stat.clone()))
            .cloned()
            .unwrap_or_default();
        let matching: Vec<&Modifier> = ids
            .iter()
            .map(|id| self.modifiers.get(id).ok_or(ModifierError::CorruptIndex))
            .collect::<Result<_, _>>()?;
        let mut value = i128::from(base.raw());
        for m in matching.iter().filter(|m| {
            m.kind == ModifierKind::Additive && m.expires_at.is_none_or(|end| now < end)
        }) {
            value = value
                .checked_add(i128::from(m.value.raw()))
                .ok_or(ModifierError::ArithmeticOverflow)?;
        }
        let mut value = i64::try_from(value).map_err(|_| ModifierError::ArithmeticOverflow)?;
        for m in matching.iter().filter(|m| {
            m.kind == ModifierKind::Multiplicative && m.expires_at.is_none_or(|end| now < end)
        }) {
            let product = i128::from(value)
                .checked_mul(i128::from(m.value.raw()))
                .ok_or(ModifierError::ArithmeticOverflow)?
                / i128::from(FIXED_SCALE);
            value = i64::try_from(product).map_err(|_| ModifierError::ArithmeticOverflow)?;
        }
        let result = Fixed::from_raw(value);
        self.cache.insert(key, result);
        Ok(result)
    }

    pub fn modifiers(&self) -> &BTreeMap<ModifierId, Modifier> {
        &self.modifiers
    }

    /// Rebuilds the derived lookup after deserializing a standalone ModifierSet.
    pub fn rebuild_index(&mut self) -> Result<(), ModifierError> {
        let mut index: BTreeMap<(EntityId, StatId), BTreeSet<ModifierId>> = BTreeMap::new();
        for (id, modifier) in &self.modifiers {
            if id != &modifier.id {
                return Err(ModifierError::CorruptIndex);
            }
            index
                .entry((modifier.target, modifier.stat.clone()))
                .or_default()
                .insert(*id);
        }
        self.by_target_stat = index;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), ModifierError> {
        let mut expected: BTreeMap<(EntityId, StatId), BTreeSet<ModifierId>> = BTreeMap::new();
        for (id, m) in &self.modifiers {
            expected
                .entry((m.target, m.stat.clone()))
                .or_default()
                .insert(*id);
            if id != &m.id {
                return Err(ModifierError::CorruptIndex);
            }
            StatId::new(m.stat.as_str())?;
            if m.source.trim().is_empty() {
                return Err(ModifierError::EmptySource);
            }
            if m.kind == ModifierKind::Multiplicative && m.value.raw() <= 0 {
                return Err(ModifierError::InvalidMultiplier(*id));
            }
        }
        if expected != self.by_target_stat {
            return Err(ModifierError::CorruptIndex);
        }
        Ok(())
    }

    fn clear_cache(&mut self) {
        self.cache.clear();
        self.cache_tick = None;
    }
}
