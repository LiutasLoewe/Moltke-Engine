-- Game-specific data can describe countries, corporations, species, or systems.
return {
  { kind = "create_entity", entity_id = 1001 },
  { kind = "add_entity_tag", entity_id = 1001, tag = "species.human" },
  {
    kind = "set_component_field",
    entity_id = 1001,
    component = "identity",
    field = "name",
    value_kind = "text",
    value = "Northland",
  },
  {
    kind = "set_component_field",
    entity_id = 1001,
    component = "government",
    field = "legitimacy",
    value_kind = "fixed",
    value_raw = 7200,
  },
}
