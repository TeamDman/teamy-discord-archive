use crate::cli::live::user::list::LiveUserListArgs;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue::{self as args};
use serenity::all::Http;

/// Live user queries.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct LiveUserArgs {
    /// The user subcommand to run.
    #[facet(args::subcommand)]
    pub command: LiveUserCommand,
}

/// User subcommands.
// cli[impl command.surface.live-user]
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum LiveUserCommand {
    /// List users in a guild.
    List(LiveUserListArgs),
}

impl LiveUserArgs {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self, http: &Http) -> Result<()> {
        match self.command {
            LiveUserCommand::List(args) => args.invoke(http).await?,
        }

        Ok(())
    }
}
