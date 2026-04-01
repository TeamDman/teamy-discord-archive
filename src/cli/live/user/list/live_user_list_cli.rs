use arbitrary::Arbitrary;
use eyre::Context;
use eyre::Result;
use facet::Facet;
use figue::{self as args};
use serenity::all::GuildId;
use serenity::all::Http;
use serenity::all::UserId;

/// List users in a guild.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct LiveUserListArgs {
    /// Guild id whose users should be listed.
    #[facet(args::named)]
    pub guild_id: u64,

    /// Start listing after this user id.
    #[facet(args::named)]
    pub after_user_id: Option<u64>,

    /// Maximum number of users to list.
    #[facet(args::named)]
    pub limit: Option<u64>,
}

impl LiveUserListArgs {
    /// # Errors
    ///
    /// This function will return an error if the Discord API call fails.
    pub async fn invoke(self, http: &Http) -> Result<()> {
        let guild_id = GuildId::new(self.guild_id);
        let users = http
            .get_guild_members(
                guild_id,
                Some(crate::discord::live::normalize_user_limit(self.limit)),
                self.after_user_id.map(UserId::new).map(UserId::get),
            )
            .await
            .wrap_err_with(|| format!("Failed to list users for guild {}", guild_id.get()))?;
        crate::json_stdout::print_serde_json(&users)
    }
}
