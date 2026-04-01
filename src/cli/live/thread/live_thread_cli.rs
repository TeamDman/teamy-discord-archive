use crate::cli::live::thread::list::LiveThreadListArgs;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue::{self as args};
use serenity::all::Http;

/// Live thread queries.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct LiveThreadArgs {
    /// The thread subcommand to run.
    #[facet(args::subcommand)]
    pub command: LiveThreadCommand,
}

/// Thread subcommands.
// cli[impl command.surface.live-thread]
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum LiveThreadCommand {
    /// List active threads in a guild.
    List(LiveThreadListArgs),
}

impl LiveThreadArgs {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self, http: &Http) -> Result<()> {
        match self.command {
            LiveThreadCommand::List(args) => args.invoke(http).await?,
        }

        Ok(())
    }
}
