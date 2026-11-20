use crate::{CountryId, CountryTag, EntityId, Fixed, Modifier, ModifierError, ModifierId, PlayerId, ProvinceId, StatId, Tick};
use serde::{Deserialize, Serialize};

/// A game-specific mutation requested for a future simulation tick.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CommandAction {
    AddCountry {
        country_id: CountryId,
        tag: CountryTag,
    },
    AddProvince {
        province_id: ProvinceId,
        owner: Option<CountryId>,
    },
    TransferProvince {
        province_id: ProvinceId,
        new_owner: Option<CountryId>,
    },
    SetCountryStability {
        country_id: CountryId,
        stability_bp: i16,
    },
    SetStat { entity: EntityId, stat: StatId, value: Fixed },
    AddModifier { modifier: Modifier },
    RemoveModifier { modifier_id: ModifierId },
}

/// Command order is tick, player ID, then sequence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QueuedCommand {
    pub tick: Tick,
    pub player_id: PlayerId,
    pub sequence: u64,
    pub action: CommandAction,
}

impl QueuedCommand {
    pub fn new(
        tick: Tick,
        player_id: PlayerId,
        sequence: u64,
        action: CommandAction,
    ) -> Self {
        Self {
            tick,
            player_id,
            sequence,
            action,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandError {
    DuplicateCountry(CountryId),
    DuplicateCountryTag(CountryTag),
    UnknownCountry(CountryId),
    DuplicateProvince(ProvinceId),
    UnknownProvince(ProvinceId),
    StabilityOutOfRange(i16),
    InvalidStatId(String),
    Modifier(ModifierError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandOutcome {
    pub tick: Tick,
    pub player_id: PlayerId,
    pub sequence: u64,
    pub result: Result<(), CommandError>,
}
