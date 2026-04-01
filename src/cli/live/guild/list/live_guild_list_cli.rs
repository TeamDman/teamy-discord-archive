use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use serenity::all::Http;

/// List guilds visible to the bot.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct LiveGuildListArgs;

impl LiveGuildListArgs {
    /// # Errors
    ///
    /// This function will return an error if the Discord API call fails.
    pub async fn invoke(self, http: &Http) -> Result<()> {
        let guilds = crate::discord::live::list_guilds(http).await?;
        crate::json_stdout::print_serde_json(&guilds)
    }
}
