use arbitrary::Arbitrary;
use eyre::Context;
use eyre::Result;
use facet::Facet;
use figue::{self as args};

/// Validate the effective Discord bot token against the live API.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct BotTokenValidateArgs {
    /// Discord bot token. If omitted, uses the environment variable or persisted preference.
    #[facet(args::named)]
    pub token: Option<String>,
}

#[derive(Facet, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
struct ValidatedBotTokenRecord {
    source: String,
    user_id: u64,
    username: String,
    global_name: Option<String>,
    bot: bool,
}

impl BotTokenValidateArgs {
    /// # Errors
    ///
    /// This function will return an error if the token cannot be resolved or the Discord API rejects it.
    // cli[impl auth.bot-token.validate-resolves-effective]
    pub async fn invoke(self) -> Result<()> {
        let resolved = crate::paths::resolve_bot_token(self.token.as_deref())?;
        let http = serenity::all::Http::new(&resolved.token);
        let current_user = http
            .get_current_user()
            .await
            .wrap_err("Failed to validate Discord bot token")?;

        let record = ValidatedBotTokenRecord {
            source: resolved.source.as_str().to_owned(),
            user_id: current_user.id.get(),
            username: current_user.name.clone(),
            global_name: current_user.global_name.clone(),
            bot: current_user.bot,
        };
        crate::json_stdout::print_facet_json(&record)
    }
}
