use arbitrary::Arbitrary;
use eyre::Context;
use eyre::Result;
use facet::Facet;
use figue::{self as args};
use serenity::all::GuildId;
use serenity::all::Http;

/// List active threads in a guild.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct LiveThreadListArgs {
    /// Guild id whose active threads should be listed.
    #[facet(args::named)]
    pub guild_id: u64,
}

impl LiveThreadListArgs {
    /// # Errors
    ///
    /// This function will return an error if the Discord API call fails.
    pub async fn invoke(self, http: &Http) -> Result<()> {
        let guild_id = GuildId::new(self.guild_id);
        let threads = http
            .get_guild_active_threads(guild_id)
            .await
            .wrap_err_with(|| {
                format!("Failed to list active threads for guild {}", guild_id.get())
            })?;
        crate::json_stdout::print_serde_json(&threads)
    }
}
