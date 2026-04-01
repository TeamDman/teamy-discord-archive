use crate::cli::live::guild::list::LiveGuildListArgs;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue::{self as args};
use serenity::all::Http;

/// Live guild queries.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct LiveGuildArgs {
    /// The guild subcommand to run.
    #[facet(args::subcommand)]
    pub command: LiveGuildCommand,
}

/// Guild subcommands.
// cli[impl command.surface.live-guild]
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum LiveGuildCommand {
    /// List guilds visible to the bot.
    List(LiveGuildListArgs),
}

impl LiveGuildArgs {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self, http: &Http) -> Result<()> {
        match self.command {
            LiveGuildCommand::List(args) => args.invoke(http).await?,
        }

        Ok(())
    }
}
