use crate::cli::live::attachment::list::LiveAttachmentListArgs;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue::{self as args};
use serenity::all::Http;

/// Live attachment queries.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct LiveAttachmentArgs {
    /// The attachment subcommand to run.
    #[facet(args::subcommand)]
    pub command: LiveAttachmentCommand,
}

/// Attachment subcommands.
// cli[impl command.surface.live-attachment]
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum LiveAttachmentCommand {
    /// List attachments visible on the fetched messages.
    List(LiveAttachmentListArgs),
}

impl LiveAttachmentArgs {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self, http: &Http) -> Result<()> {
        match self.command {
            LiveAttachmentCommand::List(args) => args.invoke(http).await?,
        }

        Ok(())
    }
}
