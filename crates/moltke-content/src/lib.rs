//! Version-neutral content loading primitives.
//!
//! Content is parsed and validated before it is handed to the simulation.
//! This crate does not execute Lua or mutate authoritative game state.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_PACKAGES: usize = 65_536;
const MAX_DEPENDENCY_DEPTH: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameManifest {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub base_locale: String,
    #[serde(default)]
    pub modules: Vec<String>,
}

impl GameManifest {
    pub fn validate(&self) -> Result<(), ContentError> {
        validate_id(&self.id)?;
        if self.version.trim().is_empty() {
            return Err(ContentError::InvalidVersion(self.id.clone()));
        }
        if !self.base_locale.is_empty() && !valid_locale(&self.base_locale) {
            return Err(ContentError::InvalidLocale(self.base_locale.clone()));
        }
        let mut modules = BTreeSet::new();
        for module in &self.modules {
            validate_id(module)?;
            if !modules.insert(module) {
                return Err(ContentError::DuplicateModule(module.clone()));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModManifest {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub engine_compatibility: Option<String>,
}

impl ModManifest {
    pub fn validate(&self) -> Result<(), ContentError> {
        validate_id(&self.id)?;
        if self.version.trim().is_empty() {
            return Err(ContentError::InvalidVersion(self.id.clone()));
        }
        let mut dependencies = BTreeSet::new();
        for dependency in &self.dependencies {
            validate_id(dependency)?;
            if dependency == &self.id {
                return Err(ContentError::SelfDependency(self.id.clone()));
            }
            if !dependencies.insert(dependency) {
                return Err(ContentError::DuplicateDependency {
                    mod_id: self.id.clone(),
                    dependency: dependency.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Stable simulation phase order. A game can add systems inside these engine
/// boundaries, while the host remains responsible for executing each system.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemPhase {
    Input,
    Events,
    Commands,
    Gameplay,
    Cleanup,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SystemDefinition {
    pub id: String,
    pub phase: SystemPhase,
    #[serde(default)]
    pub enabled_by_default: bool,
    #[serde(default)]
    pub after: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SystemRegistry {
    ordered: Vec<SystemDefinition>,
}

impl SystemRegistry {
    pub fn new(
        definitions: impl IntoIterator<Item = SystemDefinition>,
    ) -> Result<Self, ContentError> {
        let mut by_id = BTreeMap::new();
        for definition in definitions {
            validate_id(&definition.id)?;
            let mut dependencies = BTreeSet::new();
            for dependency in &definition.after {
                validate_id(dependency)?;
                if dependency == &definition.id || !dependencies.insert(dependency) {
                    return Err(ContentError::InvalidSystemDependency {
                        system: definition.id.clone(),
                        dependency: dependency.clone(),
                    });
                }
            }
            let id = definition.id.clone();
            if by_id.insert(id.clone(), definition).is_some() {
                return Err(ContentError::DuplicateSystem(id));
            }
            if by_id.len() > MAX_SYSTEMS {
                return Err(ContentError::TooManySystems(by_id.len()));
            }
        }

        for definition in by_id.values() {
            for dependency in &definition.after {
                let Some(previous) = by_id.get(dependency) else {
                    return Err(ContentError::UnknownSystemDependency {
                        system: definition.id.clone(),
                        dependency: dependency.clone(),
                    });
                };
                if previous.phase > definition.phase {
                    return Err(ContentError::SystemPhaseInversion {
                        system: definition.id.clone(),
                        dependency: dependency.clone(),
                    });
                }
            }
        }

        let mut ordered = Vec::with_capacity(by_id.len());
        for phase in [
            SystemPhase::Input,
            SystemPhase::Events,
            SystemPhase::Commands,
            SystemPhase::Gameplay,
            SystemPhase::Cleanup,
        ] {
            let mut indegree: BTreeMap<String, usize> = by_id
                .values()
                .filter(|definition| definition.phase == phase)
                .map(|definition| (definition.id.clone(), 0))
                .collect();
            let mut successors: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            for definition in by_id
                .values()
                .filter(|definition| definition.phase == phase)
            {
                for dependency in &definition.after {
                    if by_id[dependency].phase == phase {
                        *indegree
                            .get_mut(&definition.id)
                            .expect("registered system has an indegree") += 1;
                        successors
                            .entry(dependency.clone())
                            .or_default()
                            .insert(definition.id.clone());
                    }
                }
            }

            let mut ready: BTreeSet<String> = indegree
                .iter()
                .filter(|(_, degree)| **degree == 0)
                .map(|(id, _)| id.clone())
                .collect();
            let phase_count = indegree.len();
            let mut scheduled = 0;
            while let Some(id) = ready.pop_first() {
                scheduled += 1;
                ordered.push(by_id[&id].clone());
                if let Some(dependents) = successors.get(&id) {
                    for dependent in dependents {
                        let degree = indegree
                            .get_mut(dependent)
                            .expect("dependent system has an indegree");
                        *degree -= 1;
                        if *degree == 0 {
                            ready.insert(dependent.clone());
                        }
                    }
                }
            }
            if scheduled != phase_count {
                let cycle = indegree
                    .into_iter()
                    .filter_map(|(id, degree)| (degree > 0).then_some(id))
                    .collect();
                return Err(ContentError::SystemDependencyCycle(cycle));
            }
        }
        Ok(Self { ordered })
    }

    pub fn from_toml(path: &str, source: &str) -> Result<Self, ContentError> {
        let file: SystemFile = parse_toml(path, source)?;
        Self::new(file.systems)
    }

    pub fn schedule(&self) -> &[SystemDefinition] {
        &self.ordered
    }

    /// Explicit enablement replaces defaults. Output preserves the validated
    /// deterministic schedule regardless of the order of requested IDs.
    pub fn enabled_schedule(
        &self,
        enabled: Option<&BTreeSet<String>>,
    ) -> Result<Vec<&SystemDefinition>, ContentError> {
        if let Some(enabled) = enabled {
            for id in enabled {
                if !self.ordered.iter().any(|definition| &definition.id == id) {
                    return Err(ContentError::UnknownSystem(id.clone()));
                }
            }
        }
        Ok(self
            .ordered
            .iter()
            .filter(|definition| {
                enabled.map_or(definition.enabled_by_default, |set| set.contains(&definition.id))
            })
            .collect())
    }
}

const MAX_SYSTEMS: usize = 4_096;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SystemFile {
    #[serde(default)]
    systems: Vec<SystemDefinition>,
}

/// Resolves enabled mods in stable topological order. Dependencies are added
/// automatically; requested mod order breaks ties between otherwise unrelated
/// packages and later packages take override precedence in the VFS.
pub fn resolve_mod_order(
    manifests: &[ModManifest],
    enabled: &[String],
) -> Result<Vec<ModManifest>, ContentError> {
    if manifests.len() > MAX_PACKAGES {
        return Err(ContentError::TooManyPackages(manifests.len()));
    }
    let mut by_id = BTreeMap::new();
    for manifest in manifests {
        manifest.validate()?;
        if by_id
            .insert(manifest.id.clone(), manifest.clone())
            .is_some()
        {
            return Err(ContentError::DuplicateMod(manifest.id.clone()));
        }
    }

    let mut roots = BTreeSet::new();
    for id in enabled {
        validate_id(id)?;
        if !roots.insert(id.as_str()) {
            return Err(ContentError::DuplicateEnabledMod(id.clone()));
        }
        if !by_id.contains_key(id) {
            return Err(ContentError::UnknownEnabledMod(id.clone()));
        }
    }

    fn visit(
        id: &str,
        by_id: &BTreeMap<String, ModManifest>,
        stack: &mut Vec<String>,
        visited: &mut BTreeSet<String>,
        ordered: &mut Vec<ModManifest>,
    ) -> Result<(), ContentError> {
        if visited.contains(id) {
            return Ok(());
        }
        if stack.len() >= MAX_DEPENDENCY_DEPTH {
            return Err(ContentError::DependencyDepthExceeded(id.to_owned()));
        }
        if let Some(start) = stack.iter().position(|item| item == id) {
            let mut cycle = stack[start..].to_vec();
            cycle.push(id.to_owned());
            return Err(ContentError::DependencyCycle(cycle));
        }
        let manifest = by_id
            .get(id)
            .ok_or_else(|| ContentError::MissingDependency {
                mod_id: stack.last().cloned().unwrap_or_default(),
                dependency: id.to_owned(),
            })?;
        stack.push(id.to_owned());
        for dependency in &manifest.dependencies {
            visit(dependency, by_id, stack, visited, ordered)?;
        }
        stack.pop();
        visited.insert(id.to_owned());
        ordered.push(manifest.clone());
        Ok(())
    }

    let mut ordered = Vec::new();
    let mut stack = Vec::new();
    let mut visited = BTreeSet::new();
    for id in enabled {
        visit(id, &by_id, &mut stack, &mut visited, &mut ordered)?;
    }
    Ok(ordered)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentFile {
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MountedFile {
    pub path: String,
    pub source_id: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverrideNotice {
    pub path: String,
    pub previous_source: String,
    pub replacement_source: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VirtualFileSystem {
    files: BTreeMap<String, MountedFile>,
    overrides: Vec<OverrideNotice>,
}

impl VirtualFileSystem {
    pub fn mount(
        &mut self,
        source_id: impl Into<String>,
        files: impl IntoIterator<Item = ContentFile>,
    ) -> Result<(), ContentError> {
        let source_id = source_id.into();
        validate_id(&source_id)?;
        let mut prepared = BTreeMap::new();
        for file in files {
            let (key, path) = normalize_content_path(&file.path)?;
            if prepared
                .insert(
                    key,
                    MountedFile {
                        path,
                        source_id: source_id.clone(),
                        bytes: file.bytes,
                    },
                )
                .is_some()
            {
                return Err(ContentError::DuplicatePathInPackage(file.path));
            }
        }

        for (key, file) in prepared {
            let path = file.path.clone();
            if let Some(previous) = self.files.insert(key, file) {
                self.overrides.push(OverrideNotice {
                    path,
                    previous_source: previous.source_id,
                    replacement_source: source_id.clone(),
                });
            }
        }
        Ok(())
    }

    pub fn mount_directory(
        &mut self,
        source_id: impl Into<String>,
        root: impl AsRef<Path>,
    ) -> Result<(), ContentError> {
        let files = read_content_directory(root.as_ref())?;
        self.mount(source_id, files)
    }

    pub fn get(&self, path: &str) -> Option<&MountedFile> {
        let (key, _) = normalize_content_path(path).ok()?;
        self.files.get(&key)
    }

    pub fn read_text(&self, path: &str) -> Result<Option<&str>, ContentError> {
        let Some(file) = self.get(path) else {
            return Ok(None);
        };
        std::str::from_utf8(&file.bytes)
            .map(Some)
            .map_err(|_| ContentError::InvalidUtf8(file.path.clone()))
    }

    pub fn files(&self) -> impl Iterator<Item = &MountedFile> {
        self.files.values()
    }

    pub fn overrides(&self) -> &[OverrideNotice] {
        &self.overrides
    }
}

/// Reads a mod/game directory in stable path order. Symbolic links are rejected
/// so a package cannot escape its declared root through a linked file.
pub fn read_content_directory(root: &Path) -> Result<Vec<ContentFile>, ContentError> {
    fn walk(
        root: &Path,
        directory: &Path,
        output: &mut Vec<ContentFile>,
    ) -> Result<(), ContentError> {
        let mut entries = fs::read_dir(directory)
            .map_err(|source| ContentError::Io {
                path: directory.to_path_buf(),
                source,
            })?
            .map(|entry| {
                entry.map_err(|source| ContentError::Io {
                    path: directory.to_path_buf(),
                    source,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            let path = entry.path();
            let kind = entry.file_type().map_err(|source| ContentError::Io {
                path: path.clone(),
                source,
            })?;
            if kind.is_symlink() {
                return Err(ContentError::SymbolicLink(path));
            }
            if kind.is_dir() {
                walk(root, &path, output)?;
            } else if kind.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|_| ContentError::PathEscapesRoot(path.clone()))?;
                let relative = relative
                    .to_str()
                    .ok_or_else(|| ContentError::NonUnicodePath(path.clone()))?
                    .replace('\\', "/");
                let bytes = fs::read(&path).map_err(|source| ContentError::Io {
                    path: path.clone(),
                    source,
                })?;
                output.push(ContentFile {
                    path: relative,
                    bytes,
                });
            }
        }
        Ok(())
    }

    let root = root.canonicalize().map_err(|source| ContentError::Io {
        path: root.to_path_buf(),
        source,
    })?;
    let mut output = Vec::new();
    walk(&root, &root, &mut output)?;
    output.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(output)
}

pub fn parse_toml<T: DeserializeOwned>(path: &str, source: &str) -> Result<T, ContentError> {
    toml::from_str(source).map_err(|source| ContentError::Toml {
        path: path.to_owned(),
        source,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalizedValue {
    pub value: String,
    pub source_id: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LocalizationCatalog {
    values: BTreeMap<(String, String), LocalizedValue>,
}

impl LocalizationCatalog {
    /// Inserts values in mod load order; a later source replaces the prior
    /// translation for the same locale and key.
    pub fn insert(
        &mut self,
        source_id: impl Into<String>,
        locale: impl Into<String>,
        entries: BTreeMap<String, String>,
    ) -> Result<(), ContentError> {
        let source_id = source_id.into();
        let locale = locale.into();
        validate_id(&source_id)?;
        if !valid_locale(&locale) {
            return Err(ContentError::InvalidLocale(locale));
        }
        for (key, value) in entries {
            if key.trim().is_empty() {
                return Err(ContentError::EmptyLocalizationKey);
            }
            self.values.insert(
                (locale.clone(), key),
                LocalizedValue {
                    value,
                    source_id: source_id.clone(),
                },
            );
        }
        Ok(())
    }

    pub fn load_toml(
        &mut self,
        source_id: impl Into<String>,
        locale: &str,
        path: &str,
        source: &str,
    ) -> Result<(), ContentError> {
        let entries: BTreeMap<String, String> = parse_toml(path, source)?;
        self.insert(source_id, locale, entries)
    }

    /// Returns the requested locale, then the game fallback, then the key so
    /// missing translations remain visible in development builds.
    pub fn resolve<'a>(&'a self, locale: &str, fallback: &str, key: &'a str) -> Cow<'a, str> {
        self.values
            .get(&(locale.to_owned(), key.to_owned()))
            .or_else(|| self.values.get(&(fallback.to_owned(), key.to_owned())))
            .map(|value| Cow::Borrowed(value.value.as_str()))
            .unwrap_or(Cow::Borrowed(key))
    }

    pub fn source(&self, locale: &str, key: &str) -> Option<&str> {
        self.values
            .get(&(locale.to_owned(), key.to_owned()))
            .map(|value| value.source_id.as_str())
    }
}

fn validate_id(value: &str) -> Result<(), ContentError> {
    let mut bytes = value.bytes();
    let first_valid = bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit());
    let rest_valid = bytes.all(|byte| {
        byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || byte == b'-'
            || byte == b'_'
            || byte == b'.'
    });
    if first_valid && rest_valid && value.len() <= 96 {
        Ok(())
    } else {
        Err(ContentError::InvalidId(value.to_owned()))
    }
}

fn valid_locale(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 24
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn normalize_content_path(path: &str) -> Result<(String, String), ContentError> {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') || !path.is_ascii() {
        return Err(ContentError::InvalidContentPath(path.to_owned()));
    }
    let segments: Vec<_> = path.split('/').collect();
    if segments.iter().any(|segment| {
        segment.is_empty()
            || *segment == "."
            || *segment == ".."
            || segment
                .bytes()
                .any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')))
    }) {
        return Err(ContentError::InvalidContentPath(path.to_owned()));
    }
    Ok((path.to_ascii_lowercase(), path.to_owned()))
}

#[derive(Debug)]
pub enum ContentError {
    InvalidId(String),
    InvalidVersion(String),
    InvalidLocale(String),
    DuplicateModule(String),
    DuplicateSystem(String),
    TooManySystems(usize),
    UnknownSystem(String),
    InvalidSystemDependency {
        system: String,
        dependency: String,
    },
    UnknownSystemDependency {
        system: String,
        dependency: String,
    },
    SystemPhaseInversion {
        system: String,
        dependency: String,
    },
    SystemDependencyCycle(Vec<String>),
    DuplicateMod(String),
    DuplicateEnabledMod(String),
    UnknownEnabledMod(String),
    DuplicateDependency {
        mod_id: String,
        dependency: String,
    },
    SelfDependency(String),
    MissingDependency {
        mod_id: String,
        dependency: String,
    },
    DependencyCycle(Vec<String>),
    DependencyDepthExceeded(String),
    TooManyPackages(usize),
    InvalidContentPath(String),
    DuplicatePathInPackage(String),
    InvalidUtf8(String),
    EmptyLocalizationKey,
    SymbolicLink(PathBuf),
    PathEscapesRoot(PathBuf),
    NonUnicodePath(PathBuf),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Toml {
        path: String,
        source: toml::de::Error,
    },
}

impl fmt::Display for ContentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(id) => write!(formatter, "invalid content identifier: {id:?}"),
            Self::InvalidVersion(id) => write!(formatter, "package {id:?} has an empty version"),
            Self::InvalidLocale(locale) => {
                write!(formatter, "invalid locale identifier: {locale:?}")
            }
            Self::DuplicateModule(module) => {
                write!(formatter, "module {module:?} is listed more than once")
            }
            Self::DuplicateSystem(id) => write!(formatter, "duplicate simulation system: {id}"),
            Self::TooManySystems(count) => {
                write!(formatter, "system count {count} exceeds the supported limit")
            }
            Self::UnknownSystem(id) => write!(formatter, "unknown simulation system: {id}"),
            Self::InvalidSystemDependency { system, dependency } => write!(
                formatter,
                "system {system:?} has an invalid dependency {dependency:?}"
            ),
            Self::UnknownSystemDependency { system, dependency } => write!(
                formatter,
                "system {system:?} depends on missing system {dependency:?}"
            ),
            Self::SystemPhaseInversion { system, dependency } => write!(
                formatter,
                "system {system:?} cannot run after later-phase system {dependency:?}"
            ),
            Self::SystemDependencyCycle(cycle) => write!(
                formatter,
                "simulation system dependency cycle: {}",
                cycle.join(", ")
            ),
            Self::DuplicateMod(id) => write!(formatter, "duplicate mod identifier: {id}"),
            Self::DuplicateEnabledMod(id) => {
                write!(formatter, "mod {id:?} is enabled more than once")
            }
            Self::UnknownEnabledMod(id) => write!(formatter, "enabled mod {id:?} has no manifest"),
            Self::DuplicateDependency { mod_id, dependency } => write!(
                formatter,
                "mod {mod_id:?} repeats dependency {dependency:?}"
            ),
            Self::SelfDependency(id) => write!(formatter, "mod {id:?} depends on itself"),
            Self::MissingDependency { mod_id, dependency } => write!(
                formatter,
                "mod {mod_id:?} requires missing mod {dependency:?}"
            ),
            Self::DependencyCycle(cycle) => {
                write!(formatter, "mod dependency cycle: {}", cycle.join(" -> "))
            }
            Self::DependencyDepthExceeded(id) => {
                write!(formatter, "mod dependency depth limit exceeded at {id:?}")
            }
            Self::TooManyPackages(count) => write!(
                formatter,
                "package count {count} exceeds the supported limit"
            ),
            Self::InvalidContentPath(path) => write!(formatter, "invalid content path: {path:?}"),
            Self::DuplicatePathInPackage(path) => {
                write!(formatter, "package contains duplicate path: {path:?}")
            }
            Self::InvalidUtf8(path) => write!(formatter, "content file is not UTF-8: {path}"),
            Self::EmptyLocalizationKey => {
                formatter.write_str("localization keys must not be empty")
            }
            Self::SymbolicLink(path) => write!(
                formatter,
                "symbolic links are not allowed in content packages: {}",
                path.display()
            ),
            Self::PathEscapesRoot(path) => write!(
                formatter,
                "content path escapes package root: {}",
                path.display()
            ),
            Self::NonUnicodePath(path) => write!(
                formatter,
                "content path is not valid Unicode: {}",
                path.display()
            ),
            Self::Io { path, source } => {
                write!(formatter, "could not read {}: {source}", path.display())
            }
            Self::Toml { path, source } => write!(formatter, "invalid TOML in {path}: {source}"),
        }
    }
}

impl std::error::Error for ContentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Toml { source, .. } => Some(source),
            _ => None,
        }
    }
}
