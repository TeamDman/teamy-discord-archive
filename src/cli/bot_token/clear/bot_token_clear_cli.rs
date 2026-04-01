use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;

/// Remove the persisted Discord bot token preference.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct BotTokenClearArgs;

#[derive(Facet, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
struct ClearedBotTokenRecord {
    preference_path: String,
    removed: bool,
}

impl BotTokenClearArgs {
    /// # Errors
    ///
    /// This function will return an error if removing the saved preference fails.
    // cli[impl auth.bot-token.clear-removes-preference]
    #[expect(clippy::unused_async)]
    pub async fn invoke(self) -> Result<()> {
        let removed = crate::paths::clear_bot_token_preference(&crate::paths::APP_HOME)?;
        let record = ClearedBotTokenRecord {
            preference_path: crate::paths::bot_token_preference_path(&crate::paths::APP_HOME)
                .display()
                .to_string(),
            removed,
        };
        crate::json_stdout::print_facet_json(&record)
    }
}
