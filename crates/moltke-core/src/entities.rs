use crate::{EntityId, Fixed};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Deterministic data values for game-defined entity components.
/// Floating-point values are deliberately excluded from authoritative state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ComponentValue {
    Integer(i64),
    Fixed(Fixed),
    Boolean(bool),
    Text(String),
    Reference(EntityId),
}

/// A game-specific entity represented by tags and named component fields.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct GenericEntity {
    pub tags: BTreeSet<String>,
    pub components: BTreeMap<String, BTreeMap<String, ComponentValue>>,
}

/// Ordered storage for genre-defined actors and regions.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntityStore {
    #[serde(default)]
    entities: BTreeMap<EntityId, GenericEntity>,
}

impl EntityStore {
    pub fn validate_key(value: &str) -> Result<(), EntityStoreError> {
        crate::entities::validate_key(value)
    }

    pub fn entities(&self) -> &BTreeMap<EntityId, GenericEntity> {
        &self.entities
    }

    pub fn get(&self, id: EntityId) -> Option<&GenericEntity> {
        self.entities.get(&id)
    }

    pub fn create(&mut self, id: EntityId) -> Result<(), EntityStoreError> {
        if id.0 == 0 {
            return Err(EntityStoreError::ReservedEntityId);
        }
        if self.entities.contains_key(&id) {
            return Err(EntityStoreError::DuplicateEntity(id));
        }
        self.entities.insert(id, GenericEntity::default());
        Ok(())
    }

    pub fn add_tag(&mut self, id: EntityId, tag: String) -> Result<(), EntityStoreError> {
        validate_key(&tag)?;
        let entity = self
            .entities
            .get_mut(&id)
            .ok_or(EntityStoreError::UnknownEntity(id))?;
        if !entity.tags.insert(tag.clone()) {
            return Err(EntityStoreError::DuplicateTag { id, tag });
        }
        Ok(())
    }

    pub fn remove_tag(&mut self, id: EntityId, tag: &str) -> Result<(), EntityStoreError> {
        validate_key(tag)?;
        let entity = self
            .entities
            .get_mut(&id)
            .ok_or(EntityStoreError::UnknownEntity(id))?;
        if !entity.tags.remove(tag) {
            return Err(EntityStoreError::UnknownTag {
                id,
                tag: tag.to_owned(),
            });
        }
        Ok(())
    }

    pub fn set_component_field(
        &mut self,
        id: EntityId,
        component: String,
        field: String,
        value: ComponentValue,
    ) -> Result<(), EntityStoreError> {
        validate_key(&component)?;
        validate_key(&field)?;
        if let ComponentValue::Reference(target) = &value
            && !self.entities.contains_key(target)
        {
            return Err(EntityStoreError::UnknownReference(*target));
        }
        let entity = self
            .entities
            .get_mut(&id)
            .ok_or(EntityStoreError::UnknownEntity(id))?;
        entity
            .components
            .entry(component)
            .or_default()
            .insert(field, value);
        Ok(())
    }

    pub fn remove_component_field(
        &mut self,
        id: EntityId,
        component: &str,
        field: &str,
    ) -> Result<(), EntityStoreError> {
        validate_key(component)?;
        validate_key(field)?;
        let entity = self
            .entities
            .get_mut(&id)
            .ok_or(EntityStoreError::UnknownEntity(id))?;
        let Some(fields) = entity.components.get_mut(component) else {
            return Err(EntityStoreError::UnknownComponentField {
                id,
                component: component.to_owned(),
                field: field.to_owned(),
            });
        };
        if fields.remove(field).is_none() {
            return Err(EntityStoreError::UnknownComponentField {
                id,
                component: component.to_owned(),
                field: field.to_owned(),
            });
        }
        if fields.is_empty() {
            entity.components.remove(component);
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), EntityStoreError> {
        for (id, entity) in &self.entities {
            if id.0 == 0 {
                return Err(EntityStoreError::ReservedEntityId);
            }
            for tag in &entity.tags {
                validate_key(tag)?;
            }
            for (component, fields) in &entity.components {
                validate_key(component)?;
                if fields.is_empty() {
                    return Err(EntityStoreError::EmptyComponent {
                        id: *id,
                        component: component.clone(),
                    });
                }
                for (field, value) in fields {
                    validate_key(field)?;
                    if let ComponentValue::Reference(target) = value
                        && !self.entities.contains_key(target)
                    {
                        return Err(EntityStoreError::UnknownReference(*target));
                    }
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn validate_key(value: &str) -> Result<(), EntityStoreError> {
    let valid = !value.is_empty()
        && value.len() <= 96
        && value.as_bytes()[0].is_ascii_lowercase()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'_' | b'-' | b'.' | b':')
        });
    if valid {
        Ok(())
    } else {
        Err(EntityStoreError::InvalidKey(value.to_owned()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntityStoreError {
    ReservedEntityId,
    DuplicateEntity(EntityId),
    UnknownEntity(EntityId),
    InvalidKey(String),
    DuplicateTag {
        id: EntityId,
        tag: String,
    },
    UnknownTag {
        id: EntityId,
        tag: String,
    },
    UnknownComponentField {
        id: EntityId,
        component: String,
        field: String,
    },
    UnknownReference(EntityId),
    EmptyComponent {
        id: EntityId,
        component: String,
    },
}

impl fmt::Display for EntityStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid entity data: {self:?}")
    }
}

impl std::error::Error for EntityStoreError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_entities_round_trip_with_stable_component_data() {
        let mut store = EntityStore::default();
        store.create(EntityId(1)).expect("create first entity");
        store.create(EntityId(2)).expect("create referenced entity");
        store
            .add_tag(EntityId(1), "species.human".to_owned())
            .expect("add tag");
        store
            .set_component_field(
                EntityId(1),
                "identity".to_owned(),
                "name".to_owned(),
                ComponentValue::Text("Northland".to_owned()),
            )
            .expect("set name");
        store
            .set_component_field(
                EntityId(1),
                "diplomacy".to_owned(),
                "capital".to_owned(),
                ComponentValue::Reference(EntityId(2)),
            )
            .expect("set reference");

        let json = serde_json::to_string(&store).expect("serialize entity store");
        let restored: EntityStore = serde_json::from_str(&json).expect("deserialize entity store");
        assert_eq!(restored, store);
        restored.validate().expect("validate restored store");
    }

    #[test]
    fn generic_entity_rejects_dangling_references() {
        let mut store = EntityStore::default();
        store.create(EntityId(1)).expect("create entity");
        let result = store.set_component_field(
            EntityId(1),
            "identity".to_owned(),
            "capital".to_owned(),
            ComponentValue::Reference(EntityId(2)),
        );
        assert_eq!(result, Err(EntityStoreError::UnknownReference(EntityId(2))));
    }
}

