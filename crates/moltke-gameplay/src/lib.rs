//! Optional, genre-neutral gameplay state and deterministic rule helpers.

use moltke_core::{
    ClockError, CommandOutcome, EdgeId, EntityId, MapGraph, Path, QueuedCommand, Simulation, Tick,
};
use moltke_events::{EventCatalog, EventEngine, EventEngineError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const CAMPAIGN_SAVE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameplayModule {
    Diplomacy,
    Economy,
    Military,
    Population,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct GameplayState {
    active_modules: BTreeSet<GameplayModule>,
    pub diplomacy: DiplomacyState,
    pub economy: EconomyState,
    pub military: MilitaryState,
}

impl GameplayState {
    pub fn enable(&mut self, module: GameplayModule) {
        self.active_modules.insert(module);
    }

    pub fn disable(&mut self, module: GameplayModule) {
        self.active_modules.remove(&module);
    }

    pub fn is_enabled(&self, module: GameplayModule) -> bool {
        self.active_modules.contains(&module)
    }

    pub fn enabled_modules(&self) -> &BTreeSet<GameplayModule> {
        &self.active_modules
    }

    pub fn validate(&self, graph: &MapGraph) -> Result<(), GameplayError> {
        self.diplomacy.validate()?;
        self.economy.validate()?;
        self.military.validate(graph)?;
        Ok(())
    }

    /// Advances enabled modules in a fixed order. Call once after the core
    /// simulation has completed the same tick.
    pub fn advance_tick(
        &mut self,
        tick: Tick,
        graph: &MapGraph,
        cost_overrides: &BTreeMap<EdgeId, Option<u32>>,
    ) -> Result<GameplayTickReport, GameplayError> {
        let plan = self.prepare_tick(tick)?;
        Ok(self.apply_tick(plan, graph, cost_overrides))
    }

    fn prepare_tick(&self, tick: Tick) -> Result<GameplayTickPlan, GameplayError> {
        let economy = self.economy.plan_tick(
            self.is_enabled(GameplayModule::Economy),
            self.is_enabled(GameplayModule::Population),
        )?;
        let expired_treaties = if self.is_enabled(GameplayModule::Diplomacy) {
            self.diplomacy
                .treaties
                .iter()
                .filter(|(_, treaty)| treaty.expires_at.is_some_and(|end| end <= tick))
                .map(|(id, _)| *id)
                .collect()
        } else {
            Vec::new()
        };
        Ok(GameplayTickPlan {
            expired_treaties,
            economy,
        })
    }

    fn apply_tick(
        &mut self,
        plan: GameplayTickPlan,
        graph: &MapGraph,
        cost_overrides: &BTreeMap<EdgeId, Option<u32>>,
    ) -> GameplayTickReport {
        let GameplayTickPlan {
            expired_treaties,
            economy,
        } = plan;
        for treaty_id in &expired_treaties {
            self.diplomacy.treaties.remove(treaty_id);
        }
        self.economy
            .commit_inventory_updates(economy.inventory_updates);
        let movement = if self.is_enabled(GameplayModule::Military) {
            self.military.advance_movement(graph, cost_overrides)
        } else {
            Vec::new()
        };
        GameplayTickReport {
            expired_treaties,
            production: economy.production,
            population_needs: economy.population_needs,
            movement,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct EconomyTickPlan {
    inventory_updates: BTreeMap<(EntityId, ResourceId), i64>,
    production: Vec<ProductionResult>,
    population_needs: Vec<NeedOutcome>,
}

#[derive(Clone, Debug)]
struct GameplayTickPlan {
    expired_treaties: Vec<u64>,
    economy: EconomyTickPlan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameplayTickReport {
    pub expired_treaties: Vec<u64>,
    pub production: Vec<ProductionResult>,
    pub population_needs: Vec<NeedOutcome>,
    pub movement: Vec<MovementOutcome>,
}

/// A campaign save combines core simulation data and optional gameplay data.
/// The inner core save retains its own migration/version contract.
pub struct Campaign {
    simulation: Simulation,
    gameplay: GameplayState,
}

impl Campaign {
    pub fn new(simulation: Simulation, gameplay: GameplayState) -> Self {
        Self {
            simulation,
            gameplay,
        }
    }

    pub fn simulation(&self) -> &Simulation {
        &self.simulation
    }

    pub fn simulation_mut(&mut self) -> &mut Simulation {
        &mut self.simulation
    }

    pub fn gameplay(&self) -> &GameplayState {
        &self.gameplay
    }

    pub fn gameplay_mut(&mut self) -> &mut GameplayState {
        &mut self.gameplay
    }

    /// Advances one integrated campaign tick in a fixed order:
    /// 1. Plan fallible gameplay updates without mutating campaign state.
    /// 2. Evaluate event triggers against the current tick and queue openings.
    /// 3. Advance core time and apply all due commands in canonical order.
    /// 4. Commit the prepared gameplay changes for the new tick.
    pub fn advance_tick(
        &mut self,
        event_engine: &EventEngine,
        events: &EventCatalog,
        movement_costs: &BTreeMap<EdgeId, Option<u32>>,
    ) -> Result<CampaignTickReport, CampaignTickError> {
        let current_tick = self.simulation.state().tick();
        let next_tick = Tick(
            current_tick
                .0
                .checked_add(1)
                .ok_or(CampaignTickError::TickOverflow)?,
        );
        let gameplay_plan = self.gameplay.prepare_tick(next_tick)?;
        let event_commands = event_engine.evaluate(&mut self.simulation, events)?;
        let command_outcomes = self.simulation.step()?;
        let gameplay = self.gameplay.apply_tick(
            gameplay_plan,
            self.simulation.state().map_graph(),
            movement_costs,
        );
        Ok(CampaignTickReport {
            tick: next_tick,
            event_commands,
            command_outcomes,
            gameplay,
        })
    }

    pub fn to_save_json(&self) -> Result<String, CampaignSaveError> {
        self.gameplay
            .validate(self.simulation.state().map_graph())
            .map_err(CampaignSaveError::Gameplay)?;
        let envelope = CampaignSaveEnvelope {
            format_version: CAMPAIGN_SAVE_VERSION,
            _engine_version: env!("CARGO_PKG_VERSION").to_owned(),
            simulation_json: self
                .simulation
                .to_save_json()
                .map_err(CampaignSaveError::Core)?,
            gameplay: self.gameplay.clone(),
        };
        serde_json::to_string_pretty(&envelope).map_err(CampaignSaveError::Json)
    }

    pub fn from_save_json(source: &str) -> Result<Self, CampaignSaveError> {
        let envelope: CampaignSaveEnvelope =
            serde_json::from_str(source).map_err(CampaignSaveError::Json)?;
        if envelope.format_version != CAMPAIGN_SAVE_VERSION {
            return Err(CampaignSaveError::UnsupportedFormat(
                envelope.format_version,
            ));
        }
        let simulation = Simulation::from_save_json(&envelope.simulation_json)
            .map_err(CampaignSaveError::Core)?;
        envelope
            .gameplay
            .validate(simulation.state().map_graph())
            .map_err(CampaignSaveError::Gameplay)?;
        Ok(Self {
            simulation,
            gameplay: envelope.gameplay,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignTickReport {
    pub tick: Tick,
    /// Event openings submitted for this tick before core commands were applied.
    pub event_commands: Vec<QueuedCommand>,
    pub command_outcomes: Vec<CommandOutcome>,
    pub gameplay: GameplayTickReport,
}

#[derive(Debug)]
pub enum CampaignTickError {
    TickOverflow,
    Core(ClockError),
    Events(EventEngineError),
    Gameplay(GameplayError),
}

impl From<ClockError> for CampaignTickError {
    fn from(error: ClockError) -> Self {
        Self::Core(error)
    }
}

impl From<EventEngineError> for CampaignTickError {
    fn from(error: EventEngineError) -> Self {
        Self::Events(error)
    }
}

impl From<GameplayError> for CampaignTickError {
    fn from(error: GameplayError) -> Self {
        Self::Gameplay(error)
    }
}

impl fmt::Display for CampaignTickError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "campaign tick failed: {self:?}")
    }
}

impl std::error::Error for CampaignTickError {}

#[derive(Serialize, Deserialize)]
struct CampaignSaveEnvelope {
    format_version: u32,
    #[serde(rename = "engine_version")]
    _engine_version: String,
    simulation_json: String,
    gameplay: GameplayState,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResourceId(String);

impl ResourceId {
    pub fn new(value: impl Into<String>) -> Result<Self, GameplayError> {
        let value = value.into();
        if valid_key(&value) {
            Ok(Self(value))
        } else {
            Err(GameplayError::InvalidIdentifier(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductionRule {
    pub id: u64,
    pub owner: EntityId,
    pub max_cycles_per_tick: u32,
    pub inputs: BTreeMap<ResourceId, u32>,
    pub outputs: BTreeMap<ResourceId, u32>,
}

impl ProductionRule {
    pub fn validate(&self) -> Result<(), GameplayError> {
        if self.id == 0 || self.max_cycles_per_tick == 0 || self.outputs.is_empty() {
            return Err(GameplayError::InvalidProductionRule(self.id));
        }
        if self.inputs.values().any(|amount| *amount == 0)
            || self.outputs.values().any(|amount| *amount == 0)
        {
            return Err(GameplayError::InvalidProductionRule(self.id));
        }
        for resource in self.inputs.keys().chain(self.outputs.keys()) {
            ResourceId::new(resource.as_str())?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PopulationGroup {
    pub id: EntityId,
    pub inventory_owner: EntityId,
    pub location: moltke_core::NodeId,
    pub size: u64,
    /// Amount required per 1,000 people per gameplay tick.
    pub needs_per_1000: BTreeMap<ResourceId, u32>,
    /// Ideology shares in basis points; total may not exceed 10,000.
    pub ideology_bp: BTreeMap<String, u16>,
}

impl PopulationGroup {
    pub fn validate(&self) -> Result<(), GameplayError> {
        let ideology_total = self
            .ideology_bp
            .values()
            .try_fold(0_u32, |total, value| total.checked_add(u32::from(*value)));
        if self.size == 0
            || self.needs_per_1000.values().any(|amount| *amount == 0)
            || self
                .needs_per_1000
                .keys()
                .any(|key| ResourceId::new(key.as_str()).is_err())
            || self.ideology_bp.keys().any(|key| !valid_key(key))
            || ideology_total.is_none_or(|total| total > 10_000)
        {
            return Err(GameplayError::InvalidPopulation(self.id));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct EconomyState {
    inventory: BTreeMap<(EntityId, ResourceId), i64>,
    production_rules: BTreeMap<u64, ProductionRule>,
    populations: BTreeMap<EntityId, PopulationGroup>,
}

impl EconomyState {
    pub fn stock(&self, owner: EntityId, resource: &ResourceId) -> i64 {
        self.inventory
            .get(&(owner, resource.clone()))
            .copied()
            .unwrap_or(0)
    }

    pub fn set_stock(
        &mut self,
        owner: EntityId,
        resource: ResourceId,
        amount: i64,
    ) -> Result<(), GameplayError> {
        if amount < 0 {
            return Err(GameplayError::NegativeStock(owner, resource));
        }
        ResourceId::new(resource.as_str())?;
        self.inventory.insert((owner, resource), amount);
        Ok(())
    }

    pub fn add_production_rule(&mut self, rule: ProductionRule) -> Result<(), GameplayError> {
        rule.validate()?;
        if self.production_rules.contains_key(&rule.id) {
            return Err(GameplayError::DuplicateProductionRule(rule.id));
        }
        self.production_rules.insert(rule.id, rule);
        Ok(())
    }

    pub fn add_population(&mut self, group: PopulationGroup) -> Result<(), GameplayError> {
        group.validate()?;
        if self.populations.contains_key(&group.id) {
            return Err(GameplayError::DuplicatePopulation(group.id));
        }
        self.populations.insert(group.id, group);
        Ok(())
    }

    /// Runs production rules in stable ID order. Each rule computes all stock
    /// deltas before applying them, preventing partial input consumption on an
    /// arithmetic error. All rules are planned before any inventory is changed.
    pub fn run_production(&mut self) -> Result<Vec<ProductionResult>, GameplayError> {
        let plan = self.plan_tick(true, false)?;
        self.commit_inventory_updates(plan.inventory_updates);
        Ok(plan.production)
    }

    /// Consumes each population's needs in stable group/resource order and
    /// reports unmet demand without making inventory negative.
    pub fn consume_population_needs(&mut self) -> Result<Vec<NeedOutcome>, GameplayError> {
        let plan = self.plan_tick(false, true)?;
        self.commit_inventory_updates(plan.inventory_updates);
        Ok(plan.population_needs)
    }

    fn plan_tick(
        &self,
        run_production: bool,
        consume_needs: bool,
    ) -> Result<EconomyTickPlan, GameplayError> {
        let mut plan = EconomyTickPlan::default();
        if run_production {
            plan.production.reserve(self.production_rules.len());
            for rule in self.production_rules.values() {
                let mut cycles = rule.max_cycles_per_tick;
                for (resource, required) in &rule.inputs {
                    let key = (rule.owner, resource.clone());
                    let available = plan
                        .inventory_updates
                        .get(&key)
                        .copied()
                        .or_else(|| self.inventory.get(&key).copied())
                        .unwrap_or(0);
                    let possible =
                        u32::try_from((available / i64::from(*required)).min(i64::from(u32::MAX)))
                            .map_err(|_| GameplayError::ArithmeticOverflow)?;
                    cycles = cycles.min(possible);
                }

                let mut deltas: BTreeMap<ResourceId, i128> = BTreeMap::new();
                for (resource, required) in &rule.inputs {
                    *deltas.entry(resource.clone()).or_default() -=
                        i128::from(*required) * i128::from(cycles);
                }
                for (resource, produced) in &rule.outputs {
                    *deltas.entry(resource.clone()).or_default() +=
                        i128::from(*produced) * i128::from(cycles);
                }

                let mut updated = Vec::with_capacity(deltas.len());
                for (resource, delta) in deltas {
                    let key = (rule.owner, resource.clone());
                    let current = plan
                        .inventory_updates
                        .get(&key)
                        .copied()
                        .or_else(|| self.inventory.get(&key).copied())
                        .unwrap_or(0);
                    let amount = i128::from(current)
                        .checked_add(delta)
                        .ok_or(GameplayError::ArithmeticOverflow)?;
                    let amount =
                        i64::try_from(amount).map_err(|_| GameplayError::ArithmeticOverflow)?;
                    if amount < 0 {
                        return Err(GameplayError::NegativeStock(rule.owner, resource));
                    }
                    updated.push((key, amount));
                }
                plan.inventory_updates.extend(updated);
                plan.production.push(ProductionResult {
                    rule_id: rule.id,
                    cycles,
                });
            }
        }

        if consume_needs {
            for group in self.populations.values() {
                for (resource, per_1000) in &group.needs_per_1000 {
                    let demand = u128::from(group.size) * u128::from(*per_1000);
                    let requested = i64::try_from(demand.div_ceil(1_000))
                        .map_err(|_| GameplayError::ArithmeticOverflow)?;
                    let key = (group.inventory_owner, resource.clone());
                    let available = plan
                        .inventory_updates
                        .get(&key)
                        .copied()
                        .or_else(|| self.inventory.get(&key).copied())
                        .unwrap_or(0);
                    let consumed = requested.min(available);
                    plan.inventory_updates.insert(key, available - consumed);
                    plan.population_needs.push(NeedOutcome {
                        population_id: group.id,
                        resource: resource.clone(),
                        requested,
                        consumed,
                        unmet: requested - consumed,
                    });
                }
            }
        }
        Ok(plan)
    }

    fn commit_inventory_updates(
        &mut self,
        updates: BTreeMap<(EntityId, ResourceId), i64>,
    ) {
        self.inventory.extend(updates);
    }

    pub fn validate(&self) -> Result<(), GameplayError> {
        for ((_, resource), amount) in &self.inventory {
            ResourceId::new(resource.as_str())?;
            if *amount < 0 {
                return Err(GameplayError::InvalidInventory(resource.clone()));
            }
        }
        for (id, rule) in &self.production_rules {
            if *id != rule.id {
                return Err(GameplayError::InvalidProductionRule(*id));
            }
            rule.validate()?;
        }
        for (id, group) in &self.populations {
            if *id != group.id {
                return Err(GameplayError::InvalidPopulation(*id));
            }
            group.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionResult {
    pub rule_id: u64,
    pub cycles: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NeedOutcome {
    pub population_id: EntityId,
    pub resource: ResourceId,
    pub requested: i64,
    pub consumed: i64,
    pub unmet: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Treaty {
    pub id: u64,
    pub kind: String,
    pub parties: BTreeSet<EntityId>,
    pub signed_at: Tick,
    pub expires_at: Option<Tick>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct War {
    pub id: u64,
    pub casus_belli: String,
    pub attackers: BTreeSet<EntityId>,
    pub defenders: BTreeSet<EntityId>,
    /// Warscore in basis points, bounded to -10,000..=10,000.
    pub warscore_bp: i16,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiplomacyState {
    relations: BTreeMap<(EntityId, EntityId), i16>,
    treaties: BTreeMap<u64, Treaty>,
    wars: BTreeMap<u64, War>,
}

impl DiplomacyState {
    pub fn relation(&self, from: EntityId, to: EntityId) -> i16 {
        self.relations.get(&(from, to)).copied().unwrap_or(0)
    }

    pub fn set_relation(
        &mut self,
        from: EntityId,
        to: EntityId,
        value: i16,
    ) -> Result<(), GameplayError> {
        if !(-10_000..=10_000).contains(&value) {
            return Err(GameplayError::RelationOutOfRange(value));
        }
        self.relations.insert((from, to), value);
        Ok(())
    }

    pub fn add_treaty(&mut self, treaty: Treaty) -> Result<(), GameplayError> {
        validate_treaty(&treaty)?;
        if self.treaties.contains_key(&treaty.id) {
            return Err(GameplayError::DuplicateTreaty(treaty.id));
        }
        self.treaties.insert(treaty.id, treaty);
        Ok(())
    }

    pub fn declare_war(&mut self, war: War) -> Result<(), GameplayError> {
        validate_war(&war)?;
        if self.wars.contains_key(&war.id) {
            return Err(GameplayError::DuplicateWar(war.id));
        }
        self.wars.insert(war.id, war);
        Ok(())
    }

    pub fn add_warscore(&mut self, war_id: u64, delta: i16) -> Result<(), GameplayError> {
        let war = self
            .wars
            .get_mut(&war_id)
            .ok_or(GameplayError::UnknownWar(war_id))?;
        war.warscore_bp =
            i16::try_from((i32::from(war.warscore_bp) + i32::from(delta)).clamp(-10_000, 10_000))
                .map_err(|_| GameplayError::ArithmeticOverflow)?;
        Ok(())
    }

    pub fn end_war(&mut self, war_id: u64) -> Result<War, GameplayError> {
        self.wars
            .remove(&war_id)
            .ok_or(GameplayError::UnknownWar(war_id))
    }

    pub fn wars(&self) -> &BTreeMap<u64, War> {
        &self.wars
    }

    pub fn treaties(&self) -> &BTreeMap<u64, Treaty> {
        &self.treaties
    }

    pub fn validate(&self) -> Result<(), GameplayError> {
        if self
            .relations
            .values()
            .any(|value| !(-10_000..=10_000).contains(value))
        {
            return Err(GameplayError::InvalidDiplomacyState);
        }
        for (id, treaty) in &self.treaties {
            if *id != treaty.id {
                return Err(GameplayError::InvalidTreaty(*id));
            }
            validate_treaty(treaty)?;
        }
        for (id, war) in &self.wars {
            if *id != war.id {
                return Err(GameplayError::InvalidWar(*id));
            }
            validate_war(war)?;
        }
        Ok(())
    }
}

fn validate_treaty(treaty: &Treaty) -> Result<(), GameplayError> {
    if treaty.id == 0
        || !valid_key(&treaty.kind)
        || treaty.parties.len() < 2
        || treaty.expires_at.is_some_and(|end| end < treaty.signed_at)
    {
        return Err(GameplayError::InvalidTreaty(treaty.id));
    }
    Ok(())
}

fn validate_war(war: &War) -> Result<(), GameplayError> {
    if war.id == 0
        || !valid_key(&war.casus_belli)
        || war.attackers.is_empty()
        || war.defenders.is_empty()
        || war
            .attackers
            .iter()
            .any(|party| war.defenders.contains(party))
        || !(-10_000..=10_000).contains(&war.warscore_bp)
    {
        return Err(GameplayError::InvalidWar(war.id));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnitStackId(pub u64);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnitStack {
    pub id: UnitStackId,
    pub owner: EntityId,
    pub location: moltke_core::NodeId,
    pub strength: u64,
    pub supply: u32,
    pub movement_per_tick: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnitOrder {
    pub path: Path,
    pub next_edge: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct MilitaryState {
    stacks: BTreeMap<UnitStackId, UnitStack>,
    orders: BTreeMap<UnitStackId, UnitOrder>,
}

impl MilitaryState {
    pub fn add_stack(&mut self, stack: UnitStack, graph: &MapGraph) -> Result<(), GameplayError> {
        if stack.id.0 == 0
            || stack.strength == 0
            || stack.movement_per_tick == 0
            || !graph.nodes().contains_key(&stack.location)
        {
            return Err(GameplayError::InvalidUnitStack(stack.id));
        }
        if self.stacks.contains_key(&stack.id) {
            return Err(GameplayError::DuplicateUnitStack(stack.id));
        }
        self.stacks.insert(stack.id, stack);
        Ok(())
    }

    pub fn order_movement(
        &mut self,
        stack_id: UnitStackId,
        path: Path,
        graph: &MapGraph,
    ) -> Result<(), GameplayError> {
        let stack = self
            .stacks
            .get(&stack_id)
            .ok_or(GameplayError::UnknownUnitStack(stack_id))?;
        if path.nodes.first() != Some(&stack.location)
            || path.nodes.len() != path.edges.len() + 1
            || path.edges.is_empty()
        {
            return Err(GameplayError::InvalidUnitPath(stack_id));
        }
        for (index, edge_id) in path.edges.iter().enumerate() {
            let edge = graph
                .edges()
                .get(edge_id)
                .ok_or(GameplayError::InvalidUnitPath(stack_id))?;
            if edge.from != path.nodes[index] || edge.to != path.nodes[index + 1] {
                return Err(GameplayError::InvalidUnitPath(stack_id));
            }
        }
        self.orders
            .insert(stack_id, UnitOrder { path, next_edge: 0 });
        Ok(())
    }

    /// Moves stacks in stable ID order. A blocked or invalidated edge cancels
    /// the remaining order; each traversed edge consumes one supply unit.
    pub fn advance_movement(
        &mut self,
        graph: &MapGraph,
        cost_overrides: &BTreeMap<EdgeId, Option<u32>>,
    ) -> Vec<MovementOutcome> {
        let ids: Vec<_> = self.orders.keys().copied().collect();
        let mut outcomes = Vec::new();
        let mut completed = Vec::new();
        let mut cancelled = Vec::new();
        for id in ids {
            let (Some(stack), Some(order)) = (self.stacks.get_mut(&id), self.orders.get_mut(&id))
            else {
                cancelled.push(id);
                continue;
            };
            let mut points = stack.movement_per_tick;
            let mut traversed = 0_u32;
            while order.next_edge < order.path.edges.len() {
                if stack.supply == 0 {
                    break;
                }
                let edge_id = order.path.edges[order.next_edge];
                let Some(edge) = graph.edges().get(&edge_id) else {
                    cancelled.push(id);
                    break;
                };
                let cost = match cost_overrides.get(&edge_id) {
                    Some(None) => {
                        cancelled.push(id);
                        break;
                    }
                    Some(Some(cost)) => *cost,
                    None => edge.base_cost,
                };
                if edge.from != stack.location || cost == 0 {
                    cancelled.push(id);
                    break;
                }
                if cost > points {
                    break;
                }
                stack.location = edge.to;
                stack.supply -= 1;
                points -= cost;
                order.next_edge += 1;
                traversed += 1;
            }
            if order.next_edge == order.path.edges.len() {
                completed.push(id);
            }
            outcomes.push(MovementOutcome {
                stack_id: id,
                location: stack.location,
                edges_traversed: traversed,
            });
        }
        for id in completed.into_iter().chain(cancelled) {
            self.orders.remove(&id);
        }
        outcomes
    }

    pub fn stacks(&self) -> &BTreeMap<UnitStackId, UnitStack> {
        &self.stacks
    }

    pub fn validate(&self, graph: &MapGraph) -> Result<(), GameplayError> {
        for (id, stack) in &self.stacks {
            if *id != stack.id
                || stack.strength == 0
                || stack.movement_per_tick == 0
                || !graph.nodes().contains_key(&stack.location)
            {
                return Err(GameplayError::InvalidUnitStack(*id));
            }
        }
        for (id, order) in &self.orders {
            let stack = self
                .stacks
                .get(id)
                .ok_or(GameplayError::InvalidUnitStack(*id))?;
            if order.path.nodes.len() != order.path.edges.len() + 1
                || order.next_edge > order.path.edges.len()
                || order.path.nodes.get(order.next_edge) != Some(&stack.location)
            {
                return Err(GameplayError::InvalidUnitPath(*id));
            }
            for (index, edge_id) in order.path.edges.iter().enumerate() {
                let edge = graph
                    .edges()
                    .get(edge_id)
                    .ok_or(GameplayError::InvalidUnitPath(*id))?;
                if edge.from != order.path.nodes[index] || edge.to != order.path.nodes[index + 1] {
                    return Err(GameplayError::InvalidUnitPath(*id));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MovementOutcome {
    pub stack_id: UnitStackId,
    pub location: moltke_core::NodeId,
    pub edges_traversed: u32,
}

#[derive(Debug)]
pub enum CampaignSaveError {
    Json(serde_json::Error),
    Core(moltke_core::SaveError),
    Gameplay(GameplayError),
    UnsupportedFormat(u32),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GameplayError {
    InvalidIdentifier(String),
    ArithmeticOverflow,
    InvalidProductionRule(u64),
    DuplicateProductionRule(u64),
    InvalidPopulation(EntityId),
    DuplicatePopulation(EntityId),
    NegativeStock(EntityId, ResourceId),
    InvalidInventory(ResourceId),
    RelationOutOfRange(i16),
    InvalidDiplomacyState,
    InvalidTreaty(u64),
    DuplicateTreaty(u64),
    InvalidWar(u64),
    DuplicateWar(u64),
    UnknownWar(u64),
    InvalidUnitStack(UnitStackId),
    DuplicateUnitStack(UnitStackId),
    UnknownUnitStack(UnitStackId),
    InvalidUnitPath(UnitStackId),
}

fn valid_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value.as_bytes()[0].is_ascii_lowercase()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
}

impl fmt::Display for GameplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid gameplay state: {self:?}")
    }
}

impl std::error::Error for GameplayError {}

impl fmt::Display for CampaignSaveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "campaign JSON error: {error}"),
            Self::Core(error) => write!(formatter, "core save error: {error}"),
            Self::Gameplay(error) => write!(formatter, "gameplay save error: {error}"),
            Self::UnsupportedFormat(version) => {
                write!(formatter, "unsupported campaign save format {version}")
            }
        }
    }
}

impl std::error::Error for CampaignSaveError {}

#[cfg(test)]
mod tests {
    use super::*;
    use moltke_core::{EventId, PlayerId, TriggerExpr};
    use moltke_events::{EventDefinition, EventOption};

    #[test]
    fn failed_production_plan_does_not_commit_earlier_rules() {
        let owner = EntityId(1);
        let grain = ResourceId::new("grain").expect("valid resource id");
        let wealth = ResourceId::new("wealth").expect("valid resource id");
        let mut economy = EconomyState::default();
        economy
            .set_stock(owner, wealth.clone(), i64::MAX)
            .expect("valid initial stock");
        economy
            .add_production_rule(ProductionRule {
                id: 1,
                owner,
                max_cycles_per_tick: 1,
                inputs: BTreeMap::new(),
                outputs: BTreeMap::from([(grain.clone(), 1)]),
            })
            .expect("valid first rule");
        economy
            .add_production_rule(ProductionRule {
                id: 2,
                owner,
                max_cycles_per_tick: 1,
                inputs: BTreeMap::new(),
                outputs: BTreeMap::from([(wealth.clone(), 1)]),
            })
            .expect("valid overflowing rule");

        assert!(matches!(
            economy.run_production(),
            Err(GameplayError::ArithmeticOverflow)
        ));
        assert_eq!(economy.stock(owner, &grain), 0);
        assert_eq!(economy.stock(owner, &wealth), i64::MAX);
    }

    #[test]
    fn campaign_tick_runs_events_core_and_gameplay_in_order() {
        let owner = EntityId(1);
        let grain = ResourceId::new("grain").expect("valid resource id");
        let bread = ResourceId::new("bread").expect("valid resource id");
        let mut gameplay = GameplayState::default();
        gameplay.enable(GameplayModule::Economy);
        gameplay.enable(GameplayModule::Population);
        gameplay
            .economy
            .set_stock(owner, grain.clone(), 1)
            .expect("valid initial stock");
        gameplay
            .economy
            .add_production_rule(ProductionRule {
                id: 1,
                owner,
                max_cycles_per_tick: 1,
                inputs: BTreeMap::from([(grain.clone(), 1)]),
                outputs: BTreeMap::from([(bread.clone(), 2)]),
            })
            .expect("valid production rule");
        gameplay
            .economy
            .add_population(PopulationGroup {
                id: EntityId(2),
                inventory_owner: owner,
                location: moltke_core::NodeId(1),
                size: 1_000,
                needs_per_1000: BTreeMap::from([(bread.clone(), 1)]),
                ideology_bp: BTreeMap::new(),
            })
            .expect("valid population");

        let event_id = EventId::new("market_opened").expect("valid event id");
        let mut catalog = EventCatalog::default();
        catalog
            .insert(EventDefinition {
                id: event_id,
                recipient: PlayerId(1),
                trigger: TriggerExpr::Always,
                once: true,
                cooldown_ticks: None,
                mean_time_to_happen_ticks: None,
                options: vec![EventOption {
                    id: "accept".to_owned(),
                    localization_key: "event.market.accept".to_owned(),
                    effects: Vec::new(),
                }],
            })
            .expect("valid event definition");

        let mut campaign = Campaign::new(Simulation::new(7), gameplay);
        let report = campaign
            .advance_tick(&EventEngine, &catalog, &BTreeMap::new())
            .expect("tick should advance");

        assert_eq!(report.tick, Tick(1));
        assert_eq!(report.event_commands.len(), 1);
        assert_eq!(report.command_outcomes.len(), 1);
        assert!(report.command_outcomes[0].result.is_ok());
        assert_eq!(campaign.simulation().state().event_runtime().pending().len(), 1);
        assert_eq!(campaign.gameplay().economy.stock(owner, &grain), 0);
        assert_eq!(campaign.gameplay().economy.stock(owner, &bread), 1);
        assert_eq!(report.gameplay.production[0].cycles, 1);
        assert_eq!(report.gameplay.population_needs[0].consumed, 1);
    }
}
