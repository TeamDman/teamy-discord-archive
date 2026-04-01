use crate::cli::bot_token::clear::BotTokenClearArgs;
use crate::cli::bot_token::set::BotTokenSetArgs;
use crate::cli::bot_token::show_source::BotTokenShowSourceArgs;
use crate::cli::bot_token::validate::BotTokenValidateArgs;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue as args;

/// Discord bot token preference commands.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct BotTokenArgs {
    /// The bot-token subcommand to run.
    #[facet(args::subcommand)]
    pub command: BotTokenCommand,
}

/// Discord bot token subcommands.
// cli[impl command.surface.bot-token]
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum BotTokenCommand {
    /// Remove the persisted Discord bot token preference.
    Clear(BotTokenClearArgs),
    /// Persist the default Discord bot token.
    Set(BotTokenSetArgs),
    /// Show which token source currently wins without printing the token.
    ShowSource(BotTokenShowSourceArgs),
    /// Validate the effective Discord bot token against the live API.
    Validate(BotTokenValidateArgs),
}

impl BotTokenArgs {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self) -> Result<()> {
        match self.command {
            BotTokenCommand::Clear(args) => args.invoke().await?,
            BotTokenCommand::Set(args) => args.invoke().await?,
            BotTokenCommand::ShowSource(args) => args.invoke().await?,
            BotTokenCommand::Validate(args) => args.invoke().await?,
        }

        Ok(())
    }
}
