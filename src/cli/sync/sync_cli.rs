use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue::{self as args};
use std::path::PathBuf;

/// Synchronize Discord content into the configured output directory.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct SyncArgs {
    /// Discord bot token. If omitted, uses the environment variable or persisted preference.
    #[facet(args::named)]
    pub token: Option<String>,

    /// Override the output directory for this run.
    #[facet(args::named)]
    pub output_dir: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedSync {
    pub output_dir: crate::paths::ResolvedOutputDir,
    pub state: crate::paths::SyncStateLayout,
}

/// # Errors
///
/// This function will return an error if no output directory can be resolved,
/// if the target directory cannot be created, or if the cache-backed sync state
/// layout cannot be prepared.
// cli[impl sync.requires-output-dir]
pub fn prepare_sync(
    app_home: &crate::paths::AppHome,
    cache_home: &crate::paths::CacheHome,
    command_line_output_dir: Option<PathBuf>,
    environment_output_dir: Option<&str>,
) -> Result<PreparedSync> {
    let output_dir = crate::paths::require_output_dir_with(
        app_home,
        command_line_output_dir,
        environment_output_dir,
    )?;
    output_dir.ensure_dir()?;
    let state = crate::paths::ensure_sync_state_layout(cache_home, output_dir.path.as_path())?;
    Ok(PreparedSync { output_dir, state })
}

impl SyncArgs {
    /// # Errors
    ///
    /// This function will return an error if no output directory or bot token can be resolved,
    /// if archive directories cannot be created, or if the Discord sync fails.
    // cli[impl sync.requires-token]
    pub async fn invoke(self) -> Result<()> {
        let environment_output_dir = std::env::var(crate::paths::OUTPUT_DIR_ENV_VAR).ok();
        let prepared = prepare_sync(
            &crate::paths::APP_HOME,
            &crate::paths::CACHE_DIR,
            self.output_dir.map(PathBuf::from),
            environment_output_dir.as_deref(),
        )?;

        let resolved_token = crate::paths::resolve_bot_token(self.token.as_deref())?;
        let mut summary = crate::archive::run_sync(
            prepared.output_dir.path.as_path(),
            &prepared.state,
            &resolved_token.token,
        )
        .await?;
        summary.output_dir = prepared.output_dir.path.display().to_string();
        summary.checkpoint_path = prepared.state.checkpoint_path.display().to_string();
        crate::json_stdout::print_facet_json(&summary)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::prepare_sync;
    use crate::paths::AppHome;
    use crate::paths::CacheHome;
    use crate::paths::OutputDirSource;
    use crate::paths::resolve_bot_token_with;
    use crate::paths::save_output_dir_preference;
    use tempfile::tempdir;

    #[test]
    // cli[verify sync.requires-output-dir]
    fn prepare_sync_uses_explicit_inputs_and_creates_state_dirs() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let app_home = AppHome(temp_dir.path().join("home"));
        let cache_home = CacheHome(temp_dir.path().join("cache"));
        let output_root = temp_dir.path().join("output");

        let prepared = prepare_sync(&app_home, &cache_home, Some(output_root.clone()), None)
            .expect("sync preparation should succeed");

        assert_eq!(prepared.output_dir.path, output_root);
        assert_eq!(prepared.output_dir.source, OutputDirSource::CommandLine);
        assert!(prepared.output_dir.path.exists());
        assert!(prepared.state.target_dir.exists());
        assert_eq!(
            prepared.state.checkpoint_path,
            prepared.state.target_dir.join("checkpoint.json")
        );
    }

    #[test]
    // cli[verify sync.requires-output-dir]
    fn prepare_sync_uses_saved_preference_without_globals() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let app_home = AppHome(temp_dir.path().join("home"));
        let cache_home = CacheHome(temp_dir.path().join("cache"));
        let saved_output_root = temp_dir.path().join("saved-output");
        save_output_dir_preference(&app_home, &saved_output_root)
            .expect("output directory preference should save");

        let prepared = prepare_sync(&app_home, &cache_home, None, None)
            .expect("sync preparation should resolve saved output dir");

        assert_eq!(prepared.output_dir.path, saved_output_root);
        assert_eq!(prepared.output_dir.source, OutputDirSource::Preference);
    }

    #[test]
    // cli[verify sync.requires-output-dir]
    fn prepare_sync_fails_without_any_output_dir_source() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let app_home = AppHome(temp_dir.path().join("home"));
        let cache_home = CacheHome(temp_dir.path().join("cache"));

        let error = prepare_sync(&app_home, &cache_home, None, None)
            .expect_err("sync preparation should fail without an output dir");

        assert!(error.to_string().contains("No output directory configured"));
    }

    #[test]
    // cli[verify sync.requires-token]
    fn sync_token_resolution_fails_without_any_token_source() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let app_home = AppHome(temp_dir.path().join("home"));

        let error = resolve_bot_token_with(&app_home, None, None)
            .expect_err("sync token resolution should fail without a token");

        assert!(
            error
                .to_string()
                .contains("No Discord bot token configured")
        );
    }
}
