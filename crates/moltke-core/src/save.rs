use crate::simulation::LoadValidationError;
use crate::Simulation;
use serde::{Deserialize, Serialize};

const SAVE_FORMAT_VERSION: u32 = 2;
const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize, Deserialize)]
struct SaveEnvelope {
    format_version: u32,
    #[serde(rename = "engine_version")]
    _engine_version: String,
    simulation: Simulation,
}

impl Simulation {
    /// Writes a versioned human-readable save. Ordered maps keep this output
    /// stable for a given engine version and state schema.
    pub fn to_save_json(&self) -> Result<String, SaveError> {
        self.validate_loaded()
            .map_err(|error| SaveError::InvalidState(format!("{error:?}")))?;
        let envelope = SaveEnvelope {
            format_version: SAVE_FORMAT_VERSION,
            _engine_version: ENGINE_VERSION.to_owned(),
            simulation: self.clone(),
        };
        serde_json::to_string_pretty(&envelope).map_err(SaveError::Json)
    }

    /// Loads a save only after checking its format version and state invariants.
    pub fn from_save_json(source: &str) -> Result<Self, SaveError> {
        let envelope: SaveEnvelope = serde_json::from_str(source).map_err(SaveError::Json)?;
        if envelope.format_version != SAVE_FORMAT_VERSION {
            return Err(SaveError::UnsupportedFormat(envelope.format_version));
        }
        envelope
            .simulation
            .validate_loaded()
            .map_err(|error| SaveError::InvalidState(format!("{error:?}")))?;
        Ok(envelope.simulation)
    }
}

#[derive(Debug)]
pub enum SaveError {
    Json(serde_json::Error),
    UnsupportedFormat(u32),
    InvalidState(String),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Json(error) => write!(formatter, "could not read or write save: {error}"),
            SaveError::UnsupportedFormat(version) => {
                write!(formatter, "unsupported save format version {version}")
            }
            SaveError::InvalidState(error) => write!(formatter, "invalid save state: {error}"),
        }
    }
}

impl std::error::Error for SaveError {}
