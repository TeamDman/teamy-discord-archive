use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue::{self as args};

/// Show which token source currently wins without printing the token.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct BotTokenShowSourceArgs {
    /// Discord bot token. If omitted, uses the environment variable or persisted preference.
    #[facet(args::named)]
    pub token: Option<String>,
}

#[derive(Facet, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
struct BotTokenSourceRecord {
    resolved_source: Option<String>,
    command_line_status: String,
    environment_status: String,
    preference_status: String,
    preference_path: String,
}

impl BotTokenShowSourceArgs {
    /// # Errors
    ///
    /// This function will return an error if reading the saved preference fails.
    #[expect(clippy::unused_async)]
    pub async fn invoke(self) -> Result<()> {
        let app_home = &crate::paths::APP_HOME;
        let environment_value = std::env::var(crate::paths::BOT_TOKEN_ENV_VAR).ok();
        let environment_token =
            crate::paths::load_bot_token_from_environment_value(environment_value.as_deref());
        let preference_token = crate::paths::load_bot_token_preference(app_home)?;
        let resolved = crate::paths::resolve_bot_token_from_sources(
            self.token.clone(),
            environment_token.clone(),
            preference_token.clone(),
        );

        let record = BotTokenSourceRecord {
            resolved_source: resolved.map(|value| value.source.as_str().to_owned()),
            command_line_status: if self.token.is_some() {
                "provided".to_owned()
            } else {
                "missing".to_owned()
            },
            environment_status: if environment_token.is_some() {
                "configured".to_owned()
            } else {
                "missing".to_owned()
            },
            preference_status: if preference_token.is_some() {
                "saved".to_owned()
            } else {
                "missing".to_owned()
            },
            preference_path: crate::paths::bot_token_preference_path(app_home)
                .display()
                .to_string(),
        };
        crate::json_stdout::print_facet_json(&record)
    }
}
