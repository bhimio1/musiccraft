//! Finds the files that sample paths written on another system refer to
//! (see `docs/spec/sample-paths.md`).
//!
//! A project made on Windows names samples like `C:\Users\Ana\kick.wav` or
//! `%FLStudioFactoryData%\Data\Patches\…`. On Linux and macOS a [`Resolver`]
//! turns such a path into candidate local paths (drive mappings, Wine
//! prefixes, mounted drives, FL Studio's data folders, case-insensitive
//! names, a search by file name) and returns the first file that exists.
//! Nothing here runs on the audio thread.

use std::collections::{HashMap, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

/// Where foreign paths may point on this computer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathConfig {
    /// FL Studio factory data folders (`%FLStudioFactoryData%`).
    pub fl_factory: Vec<PathBuf>,
    /// FL Studio user data folders (`%FLStudioUserData%`).
    pub fl_user: Vec<PathBuf>,
    /// Windows path prefixes (`C:` or `C:/Users/Ana/Music`) and the folders
    /// they stand for.
    pub drives: Vec<(String, PathBuf)>,
    /// Wine prefixes (folders holding `drive_c`).
    pub wine_prefixes: Vec<PathBuf>,
    /// The user's home folder.
    pub home: Option<PathBuf>,
    /// Folders searched by file name as a last resort.
    pub search: Vec<PathBuf>,
    /// Whether `/mnt/<letter>` is tried for a drive letter.
    pub mounts: bool,
}

/// User settings that refine what is detected.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overrides {
    /// FL Studio factory data folder.
    pub fl_factory: Option<PathBuf>,
    /// FL Studio user data folder.
    pub fl_user: Option<PathBuf>,
    /// Drive mappings, as in [`PathConfig::drives`].
    pub drives: Vec<(String, PathBuf)>,
    /// Extra folders to search by file name.
    pub search: Vec<PathBuf>,
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Sub-folders of `dir` (sorted), or none.
fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    out.sort();
    out
}

/// Parses `C:=/mnt/win;D:/Samples=/home/ana/Samples`.
fn parse_drive_map(text: &str) -> Vec<(String, PathBuf)> {
    text.split(';')
        .filter_map(|pair| pair.split_once('='))
        .map(|(from, to)| (normalize_prefix(from), PathBuf::from(to.trim())))
        .filter(|(from, _)| !from.is_empty())
        .collect()
}

/// A Windows prefix in the form the matcher compares: forward slashes, lower
/// case, no trailing slash.
#[must_use]
pub fn normalize_prefix(prefix: &str) -> String {
    prefix
        .trim()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
}

impl PathConfig {
    /// What this computer offers: environment variables, Wine prefixes and
    /// FL Studio folders inside them.
    #[must_use]
    pub fn detect() -> Self {
        let home = env_path("HOME");
        let mut config = Self {
            home: home.clone(),
            mounts: true,
            ..Self::default()
        };
        config.fl_factory.extend(env_path("MUSICCRAFT_FL_FACTORY"));
        config.fl_user.extend(env_path("MUSICCRAFT_FL_USER"));
        if let Some(map) = std::env::var_os("MUSICCRAFT_DRIVE_MAP") {
            config.drives = parse_drive_map(&map.to_string_lossy());
        }
        let mut prefixes: Vec<PathBuf> = env_path("WINEPREFIX").into_iter().collect();
        if let Some(home) = &home {
            prefixes.push(home.join(".wine"));
            prefixes.extend(subdirs(&home.join(".local/share/bottles/bottles")));
        }
        prefixes.retain(|p| p.join("drive_c").is_dir());
        prefixes.dedup();
        for prefix in &prefixes {
            let c = prefix.join("drive_c");
            for programs in ["Program Files", "Program Files (x86)"] {
                config.fl_factory.extend(
                    subdirs(&c.join(programs).join("Image-Line"))
                        .into_iter()
                        .filter(|d| {
                            d.file_name()
                                .is_some_and(|n| n.to_string_lossy().starts_with("FL Studio"))
                        }),
                );
            }
            for user in subdirs(&c.join("users")) {
                let data = user.join("Documents/Image-Line/FL Studio");
                if data.is_dir() {
                    config.fl_user.push(data);
                }
            }
        }
        if let Some(home) = &home {
            let data = home.join("Documents/Image-Line/FL Studio");
            if data.is_dir() {
                config.fl_user.push(data);
            }
        }
        config.wine_prefixes = prefixes;
        config
    }

    /// This configuration with user settings first.
    #[must_use]
    pub fn with_overrides(mut self, overrides: &Overrides) -> Self {
        if let Some(dir) = &overrides.fl_factory {
            self.fl_factory.insert(0, dir.clone());
        }
        if let Some(dir) = &overrides.fl_user {
            self.fl_user.insert(0, dir.clone());
        }
        let mut drives: Vec<(String, PathBuf)> = overrides
            .drives
            .iter()
            .map(|(from, to)| (normalize_prefix(from), to.clone()))
            .filter(|(from, _)| !from.is_empty())
            .collect();
        drives.append(&mut self.drives);
        self.drives = drives;
        self.search.extend(overrides.search.iter().cloned());
        self
    }
}

/// The process-wide configuration and how many times it changed.
static CONFIG: Mutex<Option<(u64, PathConfig)>> = Mutex::new(None);

/// Sets the configuration every [`Resolver`] uses from now on.
pub fn configure(config: PathConfig) {
    if let Ok(mut slot) = CONFIG.lock() {
        let generation = slot.as_ref().map_or(0, |(g, _)| g + 1);
        *slot = Some((generation, config));
    }
}

/// The current configuration and its generation; detected on first use.
#[must_use]
pub fn current() -> (u64, PathConfig) {
    match CONFIG.lock() {
        Ok(mut slot) => slot
            .get_or_insert_with(|| (0, PathConfig::detect()))
            .clone(),
        Err(_) => (0, PathConfig::detect()),
    }
}

/// Whether `raw` is written for another system: a drive letter, a
/// backslash or a `%NAME%` placeholder.
#[must_use]
pub fn is_foreign(raw: &str) -> bool {
    drive(raw).is_some() || raw.contains('\\') || placeholder(raw).is_some()
}

/// A leading drive letter and the rest of the path (slashes forward, no
/// leading slash).
fn drive(raw: &str) -> Option<(char, String)> {
    let mut chars = raw.chars();
    let letter = chars.next().filter(char::is_ascii_alphabetic)?;
    if chars.next() != Some(':') {
        return None;
    }
    let rest = chars.as_str().replace('\\', "/");
    Some((
        letter.to_ascii_lowercase(),
        rest.trim_start_matches('/').to_owned(),
    ))
}

/// A leading `%NAME%` and the rest of the path.
fn placeholder(raw: &str) -> Option<(String, String)> {
    let inner = raw.strip_prefix('%')?;
    let (name, rest) = inner.split_once('%')?;
    if name.is_empty() || name.contains(['/', '\\']) {
        return None;
    }
    let rest = rest.replace('\\', "/");
    Some((name.to_lowercase(), rest.trim_start_matches('/').to_owned()))
}

/// The local paths `raw` may stand for, best first (they need not exist).
#[must_use]
pub fn candidates(raw: &str, base: Option<&Path>, config: &PathConfig) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some((name, rest)) = placeholder(raw) {
        let roots: Vec<PathBuf> = match name.as_str() {
            "flstudiofactorydata" => config.fl_factory.clone(),
            "flstudiouserdata" => config.fl_user.clone(),
            "userprofile" => config
                .home
                .iter()
                .cloned()
                .chain(
                    config
                        .wine_prefixes
                        .iter()
                        .flat_map(|p| subdirs(&p.join("drive_c/users"))),
                )
                .collect(),
            _ => Vec::new(),
        };
        out.extend(roots.iter().map(|root| root.join(&rest)));
        return out;
    }
    if let Some((letter, rest)) = drive(raw) {
        let full = format!("{letter}:/{rest}");
        let lower = full.to_lowercase();
        // The longest matching mapping wins.
        let mut drives: Vec<&(String, PathBuf)> = config
            .drives
            .iter()
            .filter(|(from, _)| {
                lower == *from
                    || lower.starts_with(from.as_str()) && lower[from.len()..].starts_with('/')
            })
            .collect();
        drives.sort_by_key(|(from, _)| std::cmp::Reverse(from.len()));
        for (from, to) in drives {
            let tail = full.get(from.len()..).unwrap_or("").trim_start_matches('/');
            out.push(to.join(tail));
        }
        for prefix in &config.wine_prefixes {
            if letter == 'c' {
                out.push(prefix.join("drive_c").join(&rest));
            }
            out.push(
                prefix
                    .join("dosdevices")
                    .join(format!("{letter}:"))
                    .join(&rest),
            );
        }
        // C:\Users\<name>\<rest> as <home>/<rest>.
        let parts: Vec<&str> = rest.splitn(3, '/').collect();
        if let ([users, _, tail], Some(home)) = (parts.as_slice(), &config.home) {
            if users.eq_ignore_ascii_case("users") {
                out.push(home.join(tail));
            }
        }
        if config.mounts {
            out.push(PathBuf::from(format!("/mnt/{letter}")).join(&rest));
        }
        return out;
    }
    // A relative path with backslashes, from the project's folder.
    let normalized = raw.replace('\\', "/");
    if let Some(base) = base {
        out.push(base.join(&normalized));
    }
    out
}

/// `path` with each part matched ignoring case where the exact name is
/// missing, if it names an existing file.
#[must_use]
pub fn find_ignoring_case(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(name) => {
                let exact = current.join(name);
                if exact.exists() {
                    current = exact;
                    continue;
                }
                let wanted = name.to_string_lossy().to_lowercase();
                let found = std::fs::read_dir(if current.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    &current
                })
                .ok()?
                .flatten()
                .find(|e| e.file_name().to_string_lossy().to_lowercase() == wanted)?;
                current = found.path();
            }
            other => current.push(other.as_os_str()),
        }
    }
    current.is_file().then_some(current)
}

/// Most folder entries a search by file name looks at.
const SEARCH_LIMIT: usize = 20_000;
/// How deep a search by file name goes.
const SEARCH_DEPTH: usize = 4;

/// The first file called `name` (ignoring case) under `dirs`, breadth first.
fn search_name(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let wanted = name.to_lowercase();
    let mut queue: VecDeque<(PathBuf, usize)> = dirs.iter().map(|d| (d.clone(), 0)).collect();
    let mut seen = 0;
    while let Some((dir, depth)) = queue.pop_front() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            seen += 1;
            if seen > SEARCH_LIMIT {
                return None;
            }
            let path = entry.path();
            if path.is_dir() {
                if depth < SEARCH_DEPTH {
                    queue.push_back((path, depth + 1));
                }
            } else if entry.file_name().to_string_lossy().to_lowercase() == wanted {
                return Some(path);
            }
        }
    }
    None
}

/// The file name at the end of a path written on either system.
fn file_name(raw: &str) -> Option<&str> {
    raw.rsplit(['/', '\\']).next().filter(|n| !n.is_empty())
}

/// Turns foreign sample paths into local files, remembering the answers.
#[derive(Debug)]
pub struct Resolver {
    active: bool,
    generation: u64,
    config: PathConfig,
    cache: HashMap<(String, Option<PathBuf>), Option<PathBuf>>,
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}

impl Resolver {
    /// A resolver using the process-wide configuration; it translates only
    /// when not running on Windows.
    #[must_use]
    pub fn new() -> Self {
        let (generation, config) = current();
        Self {
            active: !cfg!(windows),
            generation,
            config,
            cache: HashMap::new(),
        }
    }

    /// A resolver with its own configuration (tests, tools).
    #[must_use]
    pub fn with_config(config: PathConfig, active: bool) -> Self {
        Self {
            active,
            generation: u64::MAX,
            config,
            cache: HashMap::new(),
        }
    }

    /// Takes up a changed process-wide configuration. Returns whether it
    /// changed (earlier answers are forgotten).
    pub fn refresh(&mut self) -> bool {
        if self.generation == u64::MAX {
            return false;
        }
        let (generation, config) = current();
        if generation == self.generation {
            return false;
        }
        self.generation = generation;
        self.config = config;
        self.cache.clear();
        true
    }

    /// Forgets earlier answers (files may have appeared).
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// The local file a foreign `raw` path refers to, if one is found;
    /// `None` for paths that need no translation or match nothing.
    pub fn resolve(&mut self, raw: &str, base: Option<&Path>) -> Option<PathBuf> {
        if !self.active || !is_foreign(raw) {
            return None;
        }
        let key = (raw.to_owned(), base.map(Path::to_path_buf));
        if let Some(found) = self.cache.get(&key) {
            return found.clone();
        }
        let found = self.find(raw, base);
        self.cache.insert(key, found.clone());
        found
    }

    fn find(&self, raw: &str, base: Option<&Path>) -> Option<PathBuf> {
        if let Some(found) = candidates(raw, base, &self.config)
            .iter()
            .find_map(|c| find_ignoring_case(c))
        {
            return Some(found);
        }
        let name = file_name(raw)?;
        let dirs: Vec<PathBuf> = base
            .map(Path::to_path_buf)
            .into_iter()
            .chain(self.config.search.iter().cloned())
            .collect();
        search_name(name, &dirs)
    }
}

#[cfg(test)]
mod tests;
