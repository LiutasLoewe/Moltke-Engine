//! A constrained Lua 5.4 runtime for future Moltke content and gameplay rules.
//!
//! Scripts read immutable ordered snapshots and return validated commands.

use mlua::{HookTriggers, Lua, Table, Value, VmState};
use moltke_core::{
    CommandAction, ComponentValue, CountryId, CountryTag, EntityId, EntityStore, Fixed, Modifier,
    ModifierId, ModifierKind, PlayerId, ProvinceId, QueuedCommand, Simulation, StatId, Tick,
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

const INSTRUCTION_CHECK_INTERVAL: u32 = 1_000;

pub struct LuaSandbox {
    lua: Lua,
    memory_limit_bytes: usize,
}

impl LuaSandbox {
    /// Creates a Lua VM with mlua's safe standard libraries and a memory cap.
    pub fn new(memory_limit_bytes: usize) -> mlua::Result<Self> {
        if memory_limit_bytes == 0 {
            return Err(mlua::Error::RuntimeError(
                "script memory limit must be greater than zero".to_owned(),
            ));
        }
        let lua = Lua::new();
        lua.set_memory_limit(memory_limit_bytes)?;

        // A lockstep script must not depend on a VM-local random sequence.
        let math: Table = lua.globals().get("math")?;
        math.set("random", Value::Nil)?;
        math.set("randomseed", Value::Nil)?;

        Ok(Self {
            lua,
            memory_limit_bytes,
        })
    }

    /// Executes one chunk with a conservative VM-instruction budget.
    ///
    /// Checks happen in blocks of up to 1,000 Lua VM instructions. Scripts can
    /// access only standard-library values and functions installed by the host.
    pub fn execute(
        &self,
        chunk_name: &str,
        source: &str,
        instruction_budget: u64,
    ) -> mlua::Result<()> {
        if instruction_budget == 0 {
            return Err(mlua::Error::RuntimeError(
                "script instruction budget must be greater than zero".to_owned(),
            ));
        }

        install_instruction_budget(&self.lua, instruction_budget)?;

        let result = self.lua.load(source).set_name(chunk_name).exec();
        self.lua.remove_hook();
        result
    }

    /// Evaluates one isolated Lua chunk against a read-only snapshot and
    /// converts its returned command array into validated, ordered engine
    /// commands. A fresh VM prevents globals from leaking between script runs.
    pub fn execute_commands(
        &self,
        chunk_name: &str,
        source: &str,
        simulation: &Simulation,
        player_id: PlayerId,
        first_sequence: u64,
        instruction_budget: u64,
    ) -> mlua::Result<Vec<QueuedCommand>> {
        if instruction_budget == 0 {
            return Err(mlua::Error::RuntimeError(
                "script instruction budget must be greater than zero".to_owned(),
            ));
        }
        let lua = Lua::new();
        lua.set_memory_limit(self.memory_limit_bytes)?;
        disable_nondeterministic_random(&lua)?;
        install_snapshot(&lua, simulation)?;
        install_instruction_budget(&lua, instruction_budget)?;

        let result = lua.load(source).set_name(chunk_name).eval::<Value>();
        lua.remove_hook();
        let result = result?;
        let Value::Table(commands) = result else {
            return Err(mlua::Error::RuntimeError(
                "simulation script must return an array of command tables".to_owned(),
            ));
        };
        let count = commands.raw_len();
        if count > 4_096 {
            return Err(mlua::Error::RuntimeError(
                "simulation script returned more than 4096 commands".to_owned(),
            ));
        }
        let next_tick =
            Tick(
                simulation.state().tick().0.checked_add(1).ok_or_else(|| {
                    mlua::Error::RuntimeError("simulation tick overflow".to_owned())
                })?,
            );
        let mut output = Vec::with_capacity(count);
        for index in 1..=count {
            let value = commands.raw_get::<Value>(index)?;
            let action = parse_command(value)?;
            let sequence = first_sequence
                .checked_add(u64::try_from(index - 1).map_err(|_| {
                    mlua::Error::RuntimeError("script command sequence overflow".to_owned())
                })?)
                .ok_or_else(|| {
                    mlua::Error::RuntimeError("script command sequence overflow".to_owned())
                })?;
            output.push(QueuedCommand::new(next_tick, player_id, sequence, action));
        }
        Ok(output)
    }
}

fn disable_nondeterministic_random(lua: &Lua) -> mlua::Result<()> {
    let math: Table = lua.globals().get("math")?;
    math.set("random", Value::Nil)?;
    math.set("randomseed", Value::Nil)?;
    Ok(())
}

fn install_instruction_budget(lua: &Lua, instruction_budget: u64) -> mlua::Result<()> {
    if instruction_budget == 0 {
        return Err(mlua::Error::RuntimeError(
            "script instruction budget must be greater than zero".to_owned(),
        ));
    }
    let interval = INSTRUCTION_CHECK_INTERVAL
        .min(instruction_budget.min(u64::from(u32::MAX)) as u32)
        .max(1);
    let allowed_checks = (instruction_budget / u64::from(interval)).max(1);
    let checks = Arc::new(AtomicU64::new(0));
    let hook_checks = Arc::clone(&checks);
    lua.set_hook(
        HookTriggers::new().every_nth_instruction(interval),
        move |_lua, _debug| {
            let completed = hook_checks.fetch_add(1, Ordering::Relaxed);
            if completed >= allowed_checks.saturating_sub(1) {
                return Err(mlua::Error::RuntimeError(
                    "script instruction budget exceeded".to_owned(),
                ));
            }
            Ok(VmState::Continue)
        },
    )
}

fn install_snapshot(lua: &Lua, simulation: &Simulation) -> mlua::Result<()> {
    let snapshot = lua.create_table()?;
    snapshot.set("tick", simulation.state().tick().0)?;
    let countries = lua.create_table()?;
    for (index, (id, country)) in simulation.state().countries().iter().enumerate() {
        let row = lua.create_table()?;
        row.set("id", id.0)?;
        row.set("tag", country.tag.as_str())?;
        row.set("stability_bp", country.stability_bp)?;
        let lua_index = i64::try_from(index + 1)
            .map_err(|_| mlua::Error::RuntimeError("country snapshot is too large".to_owned()))?;
        countries.raw_set(lua_index, row)?;
    }
    snapshot.set("countries", countries)?;

    let provinces = lua.create_table()?;
    for (index, (id, province)) in simulation.state().provinces().iter().enumerate() {
        let row = lua.create_table()?;
        row.set("id", id.0)?;
        if let Some(owner) = province.owner {
            row.set("owner_id", owner.0)?;
        }
        let lua_index = i64::try_from(index + 1)
            .map_err(|_| mlua::Error::RuntimeError("province snapshot is too large".to_owned()))?;
        provinces.raw_set(lua_index, row)?;
    }
    snapshot.set("provinces", provinces)?;

    let entities = lua.create_table()?;
    for (index, (id, entity)) in simulation.state().entities().entities().iter().enumerate() {
        let row = lua.create_table()?;
        row.set("id", id.0)?;
        let tags = lua.create_table()?;
        for (tag_index, tag) in entity.tags.iter().enumerate() {
            tags.raw_set(
                i64::try_from(tag_index + 1).map_err(|_| {
                    mlua::Error::RuntimeError("entity tag list is too large".to_owned())
                })?,
                tag.as_str(),
            )?;
        }
        row.set("tags", tags)?;
        let components = lua.create_table()?;
        for (component_name, fields) in &entity.components {
            let component = lua.create_table()?;
            for (field_name, value) in fields {
                component.set(field_name.as_str(), component_value_to_lua(lua, value)?)?;
            }
            components.set(component_name.as_str(), component)?;
        }
        row.set("components", components)?;
        entities.raw_set(
            i64::try_from(index + 1).map_err(|_| {
                mlua::Error::RuntimeError("entity snapshot is too large".to_owned())
            })?,
            row,
        )?;
    }
    snapshot.set("entities", entities)?;
    lua.globals().set("snapshot", snapshot)
}

fn component_value_to_lua(lua: &Lua, value: &ComponentValue) -> mlua::Result<Table> {
    let result = lua.create_table()?;
    match value {
        ComponentValue::Integer(value) => {
            result.set("kind", "integer")?;
            result.set("value", *value)?;
        }
        ComponentValue::Fixed(value) => {
            result.set("kind", "fixed")?;
            result.set("value_raw", value.raw())?;
        }
        ComponentValue::Boolean(value) => {
            result.set("kind", "boolean")?;
            result.set("value", *value)?;
        }
        ComponentValue::Text(value) => {
            result.set("kind", "text")?;
            result.set("value", value.as_str())?;
        }
        ComponentValue::Reference(value) => {
            result.set("kind", "reference")?;
            result.set("entity_id", value.0)?;
        }
    }
    Ok(result)
}

fn parse_command(value: Value) -> mlua::Result<CommandAction> {
    let Value::Table(command) = value else {
        return Err(mlua::Error::RuntimeError(
            "each returned command must be a table".to_owned(),
        ));
    };
    let kind: String = command.get("kind")?;
    match kind.as_str() {
        "create_entity" => Ok(CommandAction::CreateEntity {
            entity_id: EntityId(command.get("entity_id")?),
        }),
        "add_entity_tag" => {
            let tag: String = command.get("tag")?;
            EntityStore::validate_key(&tag)
                .map_err(|error| mlua::Error::RuntimeError(error.to_string()))?;
            Ok(CommandAction::AddEntityTag {
                entity_id: EntityId(command.get("entity_id")?),
                tag,
            })
        }
        "remove_entity_tag" => {
            let tag: String = command.get("tag")?;
            EntityStore::validate_key(&tag)
                .map_err(|error| mlua::Error::RuntimeError(error.to_string()))?;
            Ok(CommandAction::RemoveEntityTag {
                entity_id: EntityId(command.get("entity_id")?),
                tag,
            })
        }
        "set_component_field" => {
            let component: String = command.get("component")?;
            let field: String = command.get("field")?;
            EntityStore::validate_key(&component)
                .and_then(|_| EntityStore::validate_key(&field))
                .map_err(|error| mlua::Error::RuntimeError(error.to_string()))?;
            let kind: String = command.get("value_kind")?;
            let value = match kind.as_str() {
                "integer" => ComponentValue::Integer(command.get("value")?),
                "fixed" => ComponentValue::Fixed(Fixed::from_raw(command.get("value_raw")?)),
                "boolean" => ComponentValue::Boolean(command.get("value")?),
                "text" => ComponentValue::Text(command.get("value")?),
                "reference" => {
                    let id = optional_u64(&command, "value_entity_id")?.ok_or_else(|| {
                        mlua::Error::RuntimeError(
                            "value_entity_id must be a nonnegative integer".to_owned(),
                        )
                    })?;
                    ComponentValue::Reference(EntityId(id))
                }
                _ => {
                    return Err(mlua::Error::RuntimeError(
                        "value_kind must be integer, fixed, boolean, text, or reference".to_owned(),
                    ));
                }
            };
            Ok(CommandAction::SetComponentField {
                entity_id: EntityId(command.get("entity_id")?),
                component,
                field,
                value,
            })
        }
        "remove_component_field" => {
            let component: String = command.get("component")?;
            let field: String = command.get("field")?;
            EntityStore::validate_key(&component)
                .and_then(|_| EntityStore::validate_key(&field))
                .map_err(|error| mlua::Error::RuntimeError(error.to_string()))?;
            Ok(CommandAction::RemoveComponentField {
                entity_id: EntityId(command.get("entity_id")?),
                component,
                field,
            })
        }
        "add_country" => {
            let country_id = CountryId(command.get("country_id")?);
            let tag: String = command.get("tag")?;
            let tag = CountryTag::new(tag)
                .map_err(|error| mlua::Error::RuntimeError(error.to_string()))?;
            Ok(CommandAction::AddCountry { country_id, tag })
        }
        "add_province" => {
            let province_id = ProvinceId(command.get("province_id")?);
            let owner = optional_u32(&command, "owner_id")?.map(CountryId);
            Ok(CommandAction::AddProvince { province_id, owner })
        }
        "transfer_province" => {
            let province_id = ProvinceId(command.get("province_id")?);
            let new_owner = optional_u32(&command, "new_owner_id")?.map(CountryId);
            Ok(CommandAction::TransferProvince {
                province_id,
                new_owner,
            })
        }
        "set_country_stability" => Ok(CommandAction::SetCountryStability {
            country_id: CountryId(command.get("country_id")?),
            stability_bp: command.get("stability_bp")?,
        }),
        "set_stat" => {
            let entity = EntityId(command.get("entity_id")?);
            let stat: String = command.get("stat")?;
            let stat = StatId::new(stat)
                .map_err(|error| mlua::Error::RuntimeError(format!("invalid stat: {error:?}")))?;
            let value = Fixed::from_raw(command.get("value_raw")?);
            Ok(CommandAction::SetStat {
                entity,
                stat,
                value,
            })
        }
        "add_modifier" => {
            let modifier_id = ModifierId(command.get("modifier_id")?);
            let target = EntityId(command.get("target_entity_id")?);
            let stat: String = command.get("stat")?;
            let stat = StatId::new(stat)
                .map_err(|error| mlua::Error::RuntimeError(format!("invalid stat: {error:?}")))?;
            let kind: String = command.get("modifier_kind")?;
            let kind = match kind.as_str() {
                "additive" => ModifierKind::Additive,
                "multiplicative" => ModifierKind::Multiplicative,
                _ => {
                    return Err(mlua::Error::RuntimeError(
                        "modifier_kind must be additive or multiplicative".to_owned(),
                    ));
                }
            };
            let source: String = command.get("source")?;
            let expires_at = optional_u64(&command, "expires_at_tick")?.map(Tick);
            Ok(CommandAction::AddModifier {
                modifier: Modifier {
                    id: modifier_id,
                    target,
                    stat,
                    kind,
                    value: Fixed::from_raw(command.get("value_raw")?),
                    source,
                    expires_at,
                },
            })
        }
        _ => Err(mlua::Error::RuntimeError(format!(
            "unsupported command kind: {kind}"
        ))),
    }
}

fn optional_u32(table: &Table, key: &str) -> mlua::Result<Option<u32>> {
    match table.get::<Value>(key)? {
        Value::Nil => Ok(None),
        Value::Integer(value) if (0..=i64::from(u32::MAX)).contains(&value) => {
            Ok(Some(value as u32))
        }
        _ => Err(mlua::Error::RuntimeError(format!(
            "{key} must be a nonnegative 32-bit integer or nil"
        ))),
    }
}

fn optional_u64(table: &Table, key: &str) -> mlua::Result<Option<u64>> {
    match table.get::<Value>(key)? {
        Value::Nil => Ok(None),
        Value::Integer(value) if value >= 0 => Ok(Some(value as u64)),
        _ => Err(mlua::Error::RuntimeError(format!(
            "{key} must be a nonnegative integer or nil"
        ))),
    }
}
