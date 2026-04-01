use crate::paths::AppHome;
use eyre::Context;
use eyre::bail;
use std::path::Path;
use std::path::PathBuf;

const OUTPUT_DIR_PREFERENCE_FILE: &str = "output-dir.txt";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutputDirSource {
    CommandLine,
    Environment,
    Preference,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedOutputDir {
    pub path: PathBuf,
    pub source: OutputDirSource,
}

impl ResolvedOutputDir {
    /// # Errors
    ///
    /// This function will return an error if creating the directory fails.
    pub fn ensure_dir(&self) -> eyre::Result<()> {
        std::fs::create_dir_all(&self.path)?;
        Ok(())
    }
}

#[must_use]
pub fn output_dir_preference_path(app_home: &AppHome) -> PathBuf {
    app_home.file_path(OUTPUT_DIR_PREFERENCE_FILE)
}

fn normalize_path_text(value: &str) -> Option<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

/// # Errors
///
/// This function will return an error if reading the saved preference fails.
pub fn load_output_dir_preference(app_home: &AppHome) -> eyre::Result<Option<PathBuf>> {
    let preference_path = output_dir_preference_path(app_home);
    if !preference_path.exists() {
        return Ok(None);
    }

    let contents = std::fs::read_to_string(&preference_path).wrap_err_with(|| {
        format!(
            "Failed to read saved output directory preference from {}",
            preference_path.display()
        )
    })?;

    Ok(normalize_path_text(&contents))
}

/// # Errors
///
/// This function will return an error if the preference directory or file cannot be written.
// cli[impl path.output-dir.set-persists-default]
pub fn save_output_dir_preference(app_home: &AppHome, output_dir: &Path) -> eyre::Result<()> {
    app_home.ensure_dir()?;
    let preference_path = output_dir_preference_path(app_home);
    std::fs::write(&preference_path, output_dir.display().to_string()).wrap_err_with(|| {
        format!(
            "Failed to write output directory preference to {}",
            preference_path.display()
        )
    })?;
    Ok(())
}

#[must_use]
// cli[impl path.output-dir.command-line-overrides-env]
// cli[impl path.output-dir.env-overrides-preference]
pub fn resolve_output_dir_from_sources(
    command_line: Option<PathBuf>,
    environment: Option<PathBuf>,
    preference: Option<PathBuf>,
) -> Option<ResolvedOutputDir> {
    if let Some(path) = command_line {
        return Some(ResolvedOutputDir {
            path,
            source: OutputDirSource::CommandLine,
        });
    }

    if let Some(path) = environment {
        return Some(ResolvedOutputDir {
            path,
            source: OutputDirSource::Environment,
        });
    }

    preference.map(|path| ResolvedOutputDir {
        path,
        source: OutputDirSource::Preference,
    })
}

#[must_use]
pub fn load_output_dir_from_environment() -> Option<PathBuf> {
    load_output_dir_from_environment_value(std::env::var(super::OUTPUT_DIR_ENV_VAR).ok().as_deref())
}

#[must_use]
pub fn load_output_dir_from_environment_value(value: Option<&str>) -> Option<PathBuf> {
    value.and_then(normalize_path_text)
}

/// # Errors
///
/// This function will return an error if the saved preference cannot be loaded
/// or if no output directory source resolves successfully.
pub fn resolve_output_dir_with(
    app_home: &AppHome,
    command_line: Option<PathBuf>,
    environment_value: Option<&str>,
) -> eyre::Result<ResolvedOutputDir> {
    let environment = load_output_dir_from_environment_value(environment_value);
    let preference = load_output_dir_preference(app_home)?;

    resolve_output_dir_from_sources(command_line, environment, preference).ok_or_else(|| {
        eyre::eyre!(
            "No output directory configured. Use `output-dir set <path>`, set `{}`, or pass `--output-dir <path>`." ,
            super::OUTPUT_DIR_ENV_VAR
        )
    })
}

/// # Errors
///
/// This function will return an error if the saved preference cannot be loaded
/// or if no output directory source resolves successfully.
pub fn resolve_output_dir(command_line: Option<PathBuf>) -> eyre::Result<ResolvedOutputDir> {
    resolve_output_dir_with(
        &super::APP_HOME,
        command_line,
        std::env::var(super::OUTPUT_DIR_ENV_VAR).ok().as_deref(),
    )
}

/// # Errors
///
/// This function will return an error if no output directory has been configured.
pub fn require_output_dir_with(
    app_home: &AppHome,
    command_line: Option<PathBuf>,
    environment_value: Option<&str>,
) -> eyre::Result<ResolvedOutputDir> {
    let resolved = resolve_output_dir_with(app_home, command_line, environment_value)?;
    if resolved.path.as_os_str().is_empty() {
        bail!("Resolved output directory is empty")
    }
    Ok(resolved)
}

/// # Errors
///
/// This function will return an error if no output directory has been configured.
pub fn require_output_dir(command_line: Option<PathBuf>) -> eyre::Result<ResolvedOutputDir> {
    require_output_dir_with(
        &super::APP_HOME,
        command_line,
        std::env::var(super::OUTPUT_DIR_ENV_VAR).ok().as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::AppHome;
    use super::OutputDirSource;
    use super::load_output_dir_from_environment_value;
    use super::load_output_dir_preference;
    use super::require_output_dir_with;
    use super::resolve_output_dir_from_sources;
    use super::save_output_dir_preference;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    // cli[verify path.output-dir.command-line-overrides-env]
    fn output_dir_resolution_prefers_command_line() {
        let resolved = resolve_output_dir_from_sources(
            Some(PathBuf::from("cli")),
            Some(PathBuf::from("env")),
            Some(PathBuf::from("saved")),
        )
        .expect("command line path should win");

        assert_eq!(resolved.path, PathBuf::from("cli"));
        assert_eq!(resolved.source, OutputDirSource::CommandLine);
    }

    #[test]
    // cli[verify path.output-dir.env-overrides-preference]
    fn output_dir_resolution_prefers_environment_over_preference() {
        let resolved = resolve_output_dir_from_sources(
            None,
            Some(PathBuf::from("env")),
            Some(PathBuf::from("saved")),
        )
        .expect("environment path should win");

        assert_eq!(resolved.path, PathBuf::from("env"));
        assert_eq!(resolved.source, OutputDirSource::Environment);
    }

    #[test]
    fn output_dir_resolution_uses_saved_preference_last() {
        let resolved = resolve_output_dir_from_sources(None, None, Some(PathBuf::from("saved")))
            .expect("saved preference should resolve");

        assert_eq!(resolved.path, PathBuf::from("saved"));
        assert_eq!(resolved.source, OutputDirSource::Preference);
    }

    #[test]
    fn output_dir_resolution_returns_none_when_unconfigured() {
        assert!(resolve_output_dir_from_sources(None, None, None).is_none());
    }

    #[test]
    fn output_dir_environment_loader_ignores_blank_values() {
        assert!(load_output_dir_from_environment_value(Some("   ")).is_none());
    }

    #[test]
    // cli[verify path.output-dir.set-persists-default]
    fn output_dir_preference_roundtrips_on_disk() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let app_home = AppHome(temp_dir.path().join("home"));
        let output_dir = temp_dir.path().join("archive-root");

        save_output_dir_preference(&app_home, &output_dir)
            .expect("output directory preference should save");

        let loaded = load_output_dir_preference(&app_home)
            .expect("output directory preference should load")
            .expect("saved output directory should exist");

        assert_eq!(loaded, output_dir);
    }

    #[test]
    fn require_output_dir_with_prefers_explicit_inputs_without_globals() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let app_home = AppHome(temp_dir.path().join("home"));
        let saved_output_dir = temp_dir.path().join("saved-output");
        save_output_dir_preference(&app_home, &saved_output_dir)
            .expect("output directory preference should save");

        let resolved = require_output_dir_with(
            &app_home,
            Some(PathBuf::from("cli-output")),
            Some("env-output"),
        )
        .expect("explicit output directory resolution should succeed");

        assert_eq!(resolved.path, PathBuf::from("cli-output"));
        assert_eq!(resolved.source, OutputDirSource::CommandLine);
    }
}
