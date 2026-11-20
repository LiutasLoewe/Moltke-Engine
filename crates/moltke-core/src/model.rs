use crate::{
    CalendarConfig, CalendarError, CountryId, CountryTag, DeterministicRng, EntityId, EntityStore,
    EntityStoreError, EventRuntimeState, EventStateError, Fixed, GraphError, MapGraph,
    ModifierError, ModifierSet, ProvinceId, StatId, Tick,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Minimal authoritative country state; gameplay modules can extend this
/// through versioned component data rather than renderer-owned objects.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Country {
    pub tag: CountryTag,
    /// Stability in basis points, from -10,000 through 10,000.
    pub stability_bp: i16,
}

/// Minimal authoritative province state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Province {
    pub owner: Option<CountryId>,
}

/// Serializable state owned by the simulation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorldState {
    pub(crate) tick: Tick,
    pub(crate) rng: DeterministicRng,
    pub(crate) countries: BTreeMap<CountryId, Country>,
    pub(crate) provinces: BTreeMap<ProvinceId, Province>,
    #[serde(default)]
    pub(crate) map: MapGraph,
    #[serde(default)]
    pub(crate) modifiers: ModifierSet,
    #[serde(default)]
    pub(crate) base_stats: BTreeMap<EntityId, BTreeMap<StatId, Fixed>>,
    #[serde(default)]
    pub(crate) event_runtime: EventRuntimeState,
    #[serde(default)]
    pub(crate) calendar: CalendarConfig,
    #[serde(default)]
    pub(crate) entity_store: EntityStore,
}

impl WorldState {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            tick: Tick(0),
            rng: DeterministicRng::seeded(seed),
            countries: BTreeMap::new(),
            provinces: BTreeMap::new(),
            map: MapGraph::default(),
            modifiers: ModifierSet::default(),
            base_stats: BTreeMap::new(),
            event_runtime: EventRuntimeState::default(),
            calendar: CalendarConfig::default(),
            entity_store: EntityStore::default(),
        }
    }

    pub fn tick(&self) -> Tick {
        self.tick
    }

    pub fn country(&self, id: CountryId) -> Option<&Country> {
        self.countries.get(&id)
    }

    pub fn province(&self, id: ProvinceId) -> Option<&Province> {
        self.provinces.get(&id)
    }

    pub fn countries(&self) -> &BTreeMap<CountryId, Country> {
        &self.countries
    }

    pub fn provinces(&self) -> &BTreeMap<ProvinceId, Province> {
        &self.provinces
    }

    pub fn map_graph(&self) -> &MapGraph {
        &self.map
    }
    pub fn map_graph_mut(&mut self) -> &mut MapGraph {
        &mut self.map
    }
    pub fn modifiers(&self) -> &ModifierSet {
        &self.modifiers
    }
    pub fn modifiers_mut(&mut self) -> &mut ModifierSet {
        &mut self.modifiers
    }
    pub fn event_runtime(&self) -> &EventRuntimeState {
        &self.event_runtime
    }
    pub fn event_runtime_mut(&mut self) -> &mut EventRuntimeState {
        &mut self.event_runtime
    }
    pub fn calendar_config(&self) -> &CalendarConfig {
        &self.calendar
    }

    pub fn entities(&self) -> &EntityStore {
        &self.entity_store
    }

    pub fn base_stat(&self, entity: EntityId, stat: &StatId) -> Fixed {
        self.base_stats
            .get(&entity)
            .and_then(|stats| stats.get(stat))
            .copied()
            .unwrap_or(Fixed::ZERO)
    }

    pub fn set_base_stat(&mut self, entity: EntityId, stat: StatId, value: Fixed) {
        self.base_stats
            .entry(entity)
            .or_default()
            .insert(stat, value);
    }

    pub fn stat_value(
        &mut self,
        entity: EntityId,
        stat: &StatId,
        now: Tick,
    ) -> Result<Fixed, ModifierError> {
        let base = self.base_stat(entity, stat);
        self.modifiers.evaluate(entity, stat, base, now)
    }

    /// Draws from the world's serialized deterministic random stream.
    pub fn random_u64(&mut self) -> u64 {
        self.rng.next_u64()
    }

    pub fn validate(&self) -> Result<(), StateValidationError> {
        self.map.validate().map_err(StateValidationError::Graph)?;
        self.modifiers
            .validate()
            .map_err(StateValidationError::Modifiers)?;
        self.event_runtime
            .validate(self.tick)
            .map_err(StateValidationError::Events)?;
        self.calendar
            .validate()
            .map_err(StateValidationError::Calendar)?;
        self.entity_store
            .validate()
            .map_err(StateValidationError::Entities)?;
        for stats in self.base_stats.values() {
            for stat in stats.keys() {
                StatId::new(stat.as_str())
                    .map_err(|_| StateValidationError::InvalidStatId(stat.as_str().to_owned()))?;
            }
        }
        let mut tags = std::collections::BTreeSet::new();

        for (id, country) in &self.countries {
            if CountryTag::new(country.tag.as_str()).is_err() {
                return Err(StateValidationError::InvalidCountryTag(*id));
            }
            if !tags.insert(country.tag.as_str()) {
                return Err(StateValidationError::DuplicateCountryTag(
                    country.tag.as_str().to_owned(),
                ));
            }
            if !(-10_000..=10_000).contains(&country.stability_bp) {
                return Err(StateValidationError::StabilityOutOfRange(*id));
            }
        }

        for (province_id, province) in &self.provinces {
            if let Some(owner) = province.owner
                && !self.countries.contains_key(&owner)
            {
                return Err(StateValidationError::MissingProvinceOwner {
                    province_id: *province_id,
                    country_id: owner,
                });
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateValidationError {
    InvalidCountryTag(CountryId),
    DuplicateCountryTag(String),
    StabilityOutOfRange(CountryId),
    Graph(GraphError),
    Modifiers(ModifierError),
    Events(EventStateError),
    Calendar(CalendarError),
    Entities(EntityStoreError),
    InvalidStatId(String),
    MissingProvinceOwner {
        province_id: ProvinceId,
        country_id: CountryId,
    },
}

impl std::fmt::Display for StateValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid simulation state: {self:?}")
    }
}

impl std::error::Error for StateValidationError {}

