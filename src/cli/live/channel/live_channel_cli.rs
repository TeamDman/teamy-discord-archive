use crate::cli::live::channel::list::LiveChannelListArgs;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue::{self as args};
use serenity::all::Http;

/// Live channel queries.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct LiveChannelArgs {
    /// The channel subcommand to run.
    #[facet(args::subcommand)]
    pub command: LiveChannelCommand,
}

/// Channel subcommands.
// cli[impl command.surface.live-channel]
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum LiveChannelCommand {
    /// List channels in a guild.
    List(LiveChannelListArgs),
}

impl LiveChannelArgs {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self, http: &Http) -> Result<()> {
        match self.command {
            LiveChannelCommand::List(args) => args.invoke(http).await?,
        }

        Ok(())
    }
}
