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

    /// Optional sync subcommand.
    #[facet(args::subcommand)]
    pub command: Option<SyncCommand>,
}

/// Nested sync commands.
// cli[impl command.surface.sync]
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum SyncCommand {
    /// Checkpoint maintenance commands.
    // cli[impl command.surface.sync-checkpoint]
    Checkpoint(SyncCheckpointArgs),
}

/// Sync checkpoint maintenance commands.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct SyncCheckpointArgs {
    /// The checkpoint subcommand to run.
    #[facet(args::subcommand)]
    pub command: SyncCheckpointCommand,
}

/// Nested sync checkpoint commands.
// cli[impl command.surface.sync-checkpoint]
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum SyncCheckpointCommand {
    /// Reconstruct a checkpoint from archived output on disk.
    Restore(SyncCheckpointRestoreArgs),
}

/// Restore a checkpoint from archived output.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct SyncCheckpointRestoreArgs {
    /// Reconstruct and compare without writing the checkpoint file.
    #[facet(args::named, default)]
    pub dry_run: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedSync {
    pub output_dir: crate::paths::ResolvedOutputDir,
    pub state: crate::paths::SyncStateLayout,
}

// cli[impl sync.checkpoint.restore.dry-run]
fn restore_sync_checkpoint(
    prepared: &PreparedSync,
    dry_run: bool,
) -> Result<crate::archive::SyncCheckpointRestoreSummary> {
    crate::archive::restore_checkpoint_from_output(
        prepared.output_dir.path.as_path(),
        &prepared.state,
        dry_run,
    )
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

        match self.command {
            None => {
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
            }
            Some(command) => {
                let summary = command.invoke(&prepared)?;
                crate::json_stdout::print_facet_json(&summary)?;
            }
        }
        Ok(())
    }
}

impl SyncCommand {
    /// # Errors
    ///
    /// This function will return an error if the sync subcommand fails.
    pub fn invoke(
        self,
        prepared: &PreparedSync,
    ) -> Result<crate::archive::SyncCheckpointRestoreSummary> {
        match self {
            SyncCommand::Checkpoint(args) => args.invoke(prepared),
        }
    }
}

impl SyncCheckpointArgs {
    /// # Errors
    ///
    /// This function will return an error if the checkpoint subcommand fails.
    pub fn invoke(
        self,
        prepared: &PreparedSync,
    ) -> Result<crate::archive::SyncCheckpointRestoreSummary> {
        match self.command {
            SyncCheckpointCommand::Restore(args) => args.invoke(prepared),
        }
    }
}

impl SyncCheckpointRestoreArgs {
    /// # Errors
    ///
    /// This function will return an error if checkpoint reconstruction or comparison fails.
    pub fn invoke(
        self,
        prepared: &PreparedSync,
    ) -> Result<crate::archive::SyncCheckpointRestoreSummary> {
        restore_sync_checkpoint(prepared, self.dry_run)
    }
}

#[cfg(test)]
mod tests {
    use super::prepare_sync;
    use super::restore_sync_checkpoint;
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

    #[test]
    // cli[verify sync.checkpoint.restore.dry-run]
    fn restore_sync_checkpoint_dry_run_compares_without_writing() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let app_home = AppHome(temp_dir.path().join("home"));
        let cache_home = CacheHome(temp_dir.path().join("cache"));
        let output_root = temp_dir.path().join("output");
        let message_dir = output_root
            .join("guilds")
            .join("99")
            .join("channels")
            .join("10")
            .join("messages");
        std::fs::create_dir_all(&message_dir).expect("message dir should exist");
        std::fs::write(
            output_root
                .join("guilds")
                .join("99")
                .join("channels")
                .join("10")
                .join("channel.json"),
            "{}",
        )
        .expect("channel metadata should write");
        std::fs::write(message_dir.join("50.json"), "hello").expect("message should write");

        let prepared = prepare_sync(&app_home, &cache_home, Some(output_root), None)
            .expect("sync preparation should succeed");
        let existing = crate::archive::SyncCheckpoint {
            version: 1,
            targets: vec![crate::archive::SyncTargetCheckpoint {
                guild_id: 99,
                channel_id: 10,
                parent_channel_id: None,
                newest_message_id: Some(999),
                oldest_message_id: Some(999),
                historical_complete: true,
                archived_message_count: Some(999),
                archived_byte_count: Some(999),
            }],
        };
        std::fs::write(
            &prepared.state.checkpoint_path,
            facet_json::to_string_pretty(&existing).expect("checkpoint should serialize"),
        )
        .expect("checkpoint should write");

        let summary =
            restore_sync_checkpoint(&prepared, true).expect("dry-run restore should succeed");

        assert!(summary.dry_run);
        assert!(summary.existing_checkpoint_found);
        assert_eq!(summary.restored_target_count, 1);
        assert_eq!(summary.restored_message_count, 1);
        assert!(summary.comparison.is_some());

        let on_disk: crate::archive::SyncCheckpoint = facet_json::from_str(
            &std::fs::read_to_string(&prepared.state.checkpoint_path)
                .expect("checkpoint should still exist"),
        )
        .expect("checkpoint should parse");
        assert_eq!(on_disk, existing);
    }
}
