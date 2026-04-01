pub mod bot_token;
pub mod cache;
pub mod facet_shape;
pub mod global_args;
pub mod home;
pub mod invite;
pub mod live;
pub mod output_dir;
pub mod sync;

use crate::cli::bot_token::BotTokenArgs;
use crate::cli::cache::CacheArgs;
use crate::cli::global_args::GlobalArgs;
use crate::cli::home::HomeArgs;
use crate::cli::invite::InviteArgs;
use crate::cli::live::LiveArgs;
use crate::cli::output_dir::OutputDirArgs;
use crate::cli::sync::SyncArgs;
use arbitrary::Arbitrary;
use eyre::Context;
use facet::Facet;
use figue::FigueBuiltins;
use figue::{self as args};

/// Archive Discord guild content to the local filesystem.
///
/// Environment variables:
/// - `TEAMY_DISCORD_ARCHIVE_HOME_DIR` overrides the resolved application home directory.
/// - `TEAMY_DISCORD_ARCHIVE_CACHE_DIR` overrides the resolved cache directory.
/// - `TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN` supplies the Discord bot token for `bot-token`, `invite`, and `live` commands.
/// - `TEAMY_DISCORD_ARCHIVE_OUTPUT_DIR` overrides the persisted output directory preference.
/// - `RUST_LOG` provides a tracing filter when `--log-filter` is omitted.
// cli[impl parser.args-consistent]
// cli[impl parser.roundtrip]
// tool[impl cli.help.describes-behavior]
// tool[impl cli.help.describes-argv]
// tool[impl cli.help.describes-environment]
#[derive(Facet, Arbitrary, Debug)]
pub struct Cli {
    /// Global arguments (`debug`, `log_filter`, `log_file`).
    #[facet(flatten)]
    pub global_args: GlobalArgs,

    /// Standard CLI options (help, version, completions).
    #[facet(flatten)]
    #[arbitrary(default)]
    // tool[impl cli.help.position-independent]
    pub builtins: FigueBuiltins,

    /// The command to run.
    #[facet(args::subcommand)]
    pub command: Command,
}

impl PartialEq for Cli {
    fn eq(&self, other: &Self) -> bool {
        // Ignore builtins in comparison since FigueBuiltins doesn't implement PartialEq
        self.global_args == other.global_args && self.command == other.command
    }
}

impl Cli {
    /// # Errors
    ///
    /// This function will return an error if the tokio runtime cannot be built or if the command fails.
    pub fn invoke(self) -> eyre::Result<()> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .wrap_err("Failed to build tokio runtime")?;
        runtime.block_on(async move { self.command.invoke().await })?;
        Ok(())
    }
}

/// The archive CLI command surface.
// cli[impl command.surface.core]
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum Command {
    /// Discord bot token preference commands.
    BotToken(BotTokenArgs),
    /// Cache-related commands.
    Cache(CacheArgs),
    /// Home-related commands.
    Home(HomeArgs),
    /// Print and optionally open the Discord bot invite URL.
    // cli[impl command.surface.invite]
    Invite(InviteArgs),
    /// Query live Discord data through the bot token.
    Live(LiveArgs),
    /// Output directory preference commands.
    OutputDir(OutputDirArgs),
    /// Synchronize Discord content into the configured output directory.
    // cli[impl command.surface.sync]
    Sync(SyncArgs),
}

impl Command {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self) -> eyre::Result<()> {
        match self {
            Command::BotToken(args) => args.invoke().await,
            Command::Cache(args) => args.invoke().await,
            Command::Home(args) => args.invoke().await,
            Command::Invite(args) => args.invoke().await,
            Command::Live(args) => args.invoke().await,
            Command::OutputDir(args) => args.invoke().await,
            Command::Sync(args) => args.invoke().await,
        }
    }
}
