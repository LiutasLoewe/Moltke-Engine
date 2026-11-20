use serde::{Deserialize, Serialize};
use std::fmt;

/// Monotonically increasing simulation step.
#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Tick(pub u64);

/// Stable engine identity for a country.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CountryId(pub u32);

/// Stable engine identity for a province.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProvinceId(pub u32);

/// Stable player identity used to order commands for the same tick.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlayerId(pub u32);

/// A content-facing country key normalized to 2-8 uppercase ASCII letters or digits.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CountryTag(String);

impl CountryTag {
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidCountryTag> {
        let value = value.into();
        let valid_len = (2..=8).contains(&value.len());
        let valid_chars = value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit());

        if valid_len && valid_chars {
            Ok(Self(value))
        } else {
            Err(InvalidCountryTag(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CountryTag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidCountryTag(pub String);

impl fmt::Display for InvalidCountryTag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "country tag must be 2-8 uppercase ASCII letters or digits: {:?}",
            self.0
        )
    }
}

impl std::error::Error for InvalidCountryTag {}
