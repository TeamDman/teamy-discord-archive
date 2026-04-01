use arbitrary::Arbitrary;
use eyre::OptionExt;
use eyre::Result;
use facet::Facet;
use figue::{self as args};

/// Persist the default Discord bot token.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct BotTokenSetArgs {
    /// The token to persist. If omitted, uses `TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN`.
    #[facet(args::positional)]
    pub token: Option<String>,
}

impl BotTokenSetArgs {
    /// # Errors
    ///
    /// This function will return an error if no token can be resolved or the preference cannot be written.
    // cli[impl auth.bot-token.set-env-fallback]
    pub fn persist(
        self,
        app_home: &crate::paths::AppHome,
        environment_value: Option<&str>,
    ) -> Result<()> {
        let token = self
            .token
            .as_deref()
            .or(environment_value)
            .ok_or_eyre(format!(
                "No Discord bot token provided. Pass `bot-token set <token>` or set `{}`.",
                crate::paths::BOT_TOKEN_ENV_VAR
            ))?;

        crate::paths::save_bot_token_preference(app_home, token)?;
        println!(
            "{}",
            crate::paths::bot_token_preference_path(app_home).display()
        );
        Ok(())
    }

    /// # Errors
    ///
    /// This function will return an error if no token can be resolved or the preference cannot be written.
    #[expect(clippy::unused_async)]
    pub async fn invoke(self) -> Result<()> {
        self.persist(
            &crate::paths::APP_HOME,
            std::env::var(crate::paths::BOT_TOKEN_ENV_VAR)
                .ok()
                .as_deref(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::BotTokenSetArgs;
    use crate::paths::AppHome;
    use crate::paths::load_bot_token_preference;
    use tempfile::tempdir;

    #[test]
    // cli[verify auth.bot-token.set-env-fallback]
    fn persist_uses_environment_when_argument_is_missing() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let app_home = AppHome(temp_dir.path().join("home"));

        BotTokenSetArgs { token: None }
            .persist(&app_home, Some("env-token"))
            .expect("environment token should persist");

        let saved = load_bot_token_preference(&app_home)
            .expect("saved token should load")
            .expect("saved token should exist");
        assert_eq!(saved, "env-token");
    }
}
