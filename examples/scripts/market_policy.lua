-- The host supplies this read-only snapshot table. Each returned command is
-- validated by moltke-modding before the host schedules it.
local commands = {}

for _, country in ipairs(snapshot.countries) do
  if country.stability_bp < 2500 then
    table.insert(commands, {
      kind = "set_country_stability",
      country_id = country.id,
      stability_bp = 3000
    })
  end
end

return commands
