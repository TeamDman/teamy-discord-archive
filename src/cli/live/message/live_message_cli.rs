use crate::cli::live::message::list::LiveMessageListArgs;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue::{self as args};
use serenity::all::Http;

/// Live message queries.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct LiveMessageArgs {
    /// The message subcommand to run.
    #[facet(args::subcommand)]
    pub command: LiveMessageCommand,
}

/// Message subcommands.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum LiveMessageCommand {
    /// List messages in a channel or thread.
    List(LiveMessageListArgs),
}

impl LiveMessageArgs {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self, http: &Http) -> Result<()> {
        match self.command {
            LiveMessageCommand::List(args) => args.invoke(http).await?,
        }

        Ok(())
    }
}
