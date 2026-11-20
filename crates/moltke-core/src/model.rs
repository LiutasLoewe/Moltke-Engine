use crate::{
    CountryId, CountryTag, DeterministicRng, GraphError, MapGraph, ModifierError, ModifierSet,
    ProvinceId, Tick,
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

    pub fn map_graph(&self) -> &MapGraph { &self.map }
    pub fn map_graph_mut(&mut self) -> &mut MapGraph { &mut self.map }
    pub fn modifiers(&self) -> &ModifierSet { &self.modifiers }
    pub fn modifiers_mut(&mut self) -> &mut ModifierSet { &mut self.modifiers }

    /// Draws from the world's serialized deterministic random stream.
    pub fn random_u64(&mut self) -> u64 {
        self.rng.next_u64()
    }

    pub fn validate(&self) -> Result<(), StateValidationError> {
        self.map.validate().map_err(StateValidationError::Graph)?;
        self.modifiers.validate().map_err(StateValidationError::Modifiers)?;
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
            if let Some(owner) = province.owner {
                if !self.countries.contains_key(&owner) {
                    return Err(StateValidationError::MissingProvinceOwner {
                        province_id: *province_id,
                        country_id: owner,
                    });
                }
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
