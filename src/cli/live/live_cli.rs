use crate::cli::live::attachment::LiveAttachmentArgs;
use crate::cli::live::channel::LiveChannelArgs;
use crate::cli::live::guild::LiveGuildArgs;
use crate::cli::live::message::LiveMessageArgs;
use crate::cli::live::thread::LiveThreadArgs;
use crate::cli::live::user::LiveUserArgs;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue::{self as args};

/// Query live Discord data through the bot token.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct LiveArgs {
    /// Discord bot token. If omitted, uses the environment variable or persisted preference.
    #[facet(args::named)]
    pub token: Option<String>,

    /// The live subcommand to run.
    #[facet(args::subcommand)]
    pub command: LiveCommand,
}

/// Nested live Discord queries.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum LiveCommand {
    /// List attachments visible through the API.
    Attachment(LiveAttachmentArgs),
    /// List channels visible in a guild.
    Channel(LiveChannelArgs),
    /// List guilds visible to the bot.
    Guild(LiveGuildArgs),
    /// List messages visible in a channel or thread.
    Message(LiveMessageArgs),
    /// List active threads visible in a guild.
    Thread(LiveThreadArgs),
    /// List users visible in a guild.
    User(LiveUserArgs),
}

impl LiveArgs {
    /// # Errors
    ///
    /// This function will return an error if the Discord token cannot be resolved or the subcommand fails.
    pub async fn invoke(self) -> Result<()> {
        let resolved = crate::paths::resolve_bot_token(self.token.as_deref())?;
        let config = crate::discord::live::LiveDiscordClientConfig {
            token: resolved.token,
        };
        let http = config.http();

        match self.command {
            LiveCommand::Attachment(args) => args.invoke(&http).await,
            LiveCommand::Channel(args) => args.invoke(&http).await,
            LiveCommand::Guild(args) => args.invoke(&http).await,
            LiveCommand::Message(args) => args.invoke(&http).await,
            LiveCommand::Thread(args) => args.invoke(&http).await,
            LiveCommand::User(args) => args.invoke(&http).await,
        }
    }
}
