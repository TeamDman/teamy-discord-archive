use crate::paths::AppHome;
use eyre::Context;
use eyre::bail;
use std::path::PathBuf;

const BOT_TOKEN_PREFERENCE_FILE: &str = "discord-bot-token.txt";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BotTokenSource {
    CommandLine,
    Environment,
    Preference,
}

impl BotTokenSource {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::CommandLine => "command-line",
            Self::Environment => "environment",
            Self::Preference => "preference",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedBotToken {
    pub token: String,
    pub source: BotTokenSource,
}

#[must_use]
pub fn bot_token_preference_path(app_home: &AppHome) -> PathBuf {
    app_home.file_path(BOT_TOKEN_PREFERENCE_FILE)
}

fn normalize_token_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

/// # Errors
///
/// This function will return an error if reading the saved preference fails.
pub fn load_bot_token_preference(app_home: &AppHome) -> eyre::Result<Option<String>> {
    let preference_path = bot_token_preference_path(app_home);
    if !preference_path.exists() {
        return Ok(None);
    }

    let contents = std::fs::read_to_string(&preference_path).wrap_err_with(|| {
        format!(
            "Failed to read saved Discord bot token preference from {}",
            preference_path.display()
        )
    })?;

    Ok(normalize_token_text(&contents))
}

/// # Errors
///
/// This function will return an error if the preference directory or file cannot be written,
/// or if the token is blank.
// cli[impl auth.bot-token.set-persists-default]
pub fn save_bot_token_preference(app_home: &AppHome, token: &str) -> eyre::Result<()> {
    let Some(token) = normalize_token_text(token) else {
        bail!("Discord bot token must not be empty")
    };

    app_home.ensure_dir()?;
    let preference_path = bot_token_preference_path(app_home);
    std::fs::write(&preference_path, token).wrap_err_with(|| {
        format!(
            "Failed to write Discord bot token preference to {}",
            preference_path.display()
        )
    })?;
    Ok(())
}

/// # Errors
///
/// This function will return an error if removing the saved preference fails.
// cli[impl auth.bot-token.clear-removes-preference]
pub fn clear_bot_token_preference(app_home: &AppHome) -> eyre::Result<bool> {
    let preference_path = bot_token_preference_path(app_home);
    if !preference_path.exists() {
        return Ok(false);
    }

    std::fs::remove_file(&preference_path).wrap_err_with(|| {
        format!(
            "Failed to remove Discord bot token preference from {}",
            preference_path.display()
        )
    })?;
    Ok(true)
}

#[must_use]
pub fn load_bot_token_from_environment_value(value: Option<&str>) -> Option<String> {
    value.and_then(normalize_token_text)
}

#[must_use]
// cli[impl auth.live-token.command-line-overrides-env]
// cli[impl auth.live-token.env]
// cli[impl auth.live-token.preference-fallback]
pub fn resolve_bot_token_from_sources(
    command_line: Option<String>,
    environment: Option<String>,
    preference: Option<String>,
) -> Option<ResolvedBotToken> {
    if let Some(token) = command_line {
        return Some(ResolvedBotToken {
            token,
            source: BotTokenSource::CommandLine,
        });
    }

    if let Some(token) = environment {
        return Some(ResolvedBotToken {
            token,
            source: BotTokenSource::Environment,
        });
    }

    preference.map(|token| ResolvedBotToken {
        token,
        source: BotTokenSource::Preference,
    })
}

/// # Errors
///
/// This function will return an error if the saved preference cannot be loaded
/// or if no token source resolves successfully.
pub fn resolve_bot_token_with(
    app_home: &AppHome,
    command_line: Option<&str>,
    environment_value: Option<&str>,
) -> eyre::Result<ResolvedBotToken> {
    let command_line = command_line.and_then(normalize_token_text);
    let environment = load_bot_token_from_environment_value(environment_value);
    let preference = load_bot_token_preference(app_home)?;

    resolve_bot_token_from_sources(command_line, environment, preference).ok_or_else(|| {
        eyre::eyre!(
            "No Discord bot token configured. Use `bot-token set <token>`, set `{}`, or pass `--token <token>`." ,
            crate::paths::BOT_TOKEN_ENV_VAR
        )
    })
}

/// # Errors
///
/// This function will return an error if the saved preference cannot be loaded
/// or if no token source resolves successfully.
pub fn resolve_bot_token(command_line: Option<&str>) -> eyre::Result<ResolvedBotToken> {
    resolve_bot_token_with(
        &crate::paths::APP_HOME,
        command_line,
        std::env::var(crate::paths::BOT_TOKEN_ENV_VAR)
            .ok()
            .as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::AppHome;
    use super::BotTokenSource;
    use super::clear_bot_token_preference;
    use super::load_bot_token_from_environment_value;
    use super::load_bot_token_preference;
    use super::resolve_bot_token_from_sources;
    use super::resolve_bot_token_with;
    use super::save_bot_token_preference;
    use tempfile::tempdir;

    #[test]
    // cli[verify auth.live-token.command-line-overrides-env]
    fn token_resolution_prefers_command_line() {
        let resolved = resolve_bot_token_from_sources(
            Some("cli-token".to_owned()),
            Some("env-token".to_owned()),
            Some("saved-token".to_owned()),
        )
        .expect("command line token should win");

        assert_eq!(resolved.token, "cli-token");
        assert_eq!(resolved.source, BotTokenSource::CommandLine);
    }

    #[test]
    // cli[verify auth.live-token.env]
    fn token_resolution_prefers_environment_over_preference() {
        let resolved = resolve_bot_token_from_sources(
            None,
            Some("env-token".to_owned()),
            Some("saved-token".to_owned()),
        )
        .expect("environment token should win");

        assert_eq!(resolved.token, "env-token");
        assert_eq!(resolved.source, BotTokenSource::Environment);
    }

    #[test]
    // cli[verify auth.live-token.preference-fallback]
    fn token_resolution_uses_saved_preference_last() {
        let resolved = resolve_bot_token_from_sources(None, None, Some("saved-token".to_owned()))
            .expect("saved token should resolve");

        assert_eq!(resolved.token, "saved-token");
        assert_eq!(resolved.source, BotTokenSource::Preference);
    }

    #[test]
    fn token_environment_loader_ignores_blank_values() {
        assert!(load_bot_token_from_environment_value(Some("   ")).is_none());
    }

    #[test]
    // cli[verify auth.bot-token.set-persists-default]
    fn token_preference_roundtrips_on_disk() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let app_home = AppHome(temp_dir.path().join("home"));

        save_bot_token_preference(&app_home, "discord-bot-token")
            .expect("bot token preference should save");

        let loaded = load_bot_token_preference(&app_home)
            .expect("bot token preference should load")
            .expect("saved token should exist");

        assert_eq!(loaded, "discord-bot-token");
    }

    #[test]
    fn resolve_bot_token_with_uses_preference_when_other_sources_missing() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let app_home = AppHome(temp_dir.path().join("home"));
        save_bot_token_preference(&app_home, "saved-token")
            .expect("bot token preference should save");

        let resolved =
            resolve_bot_token_with(&app_home, None, None).expect("saved token should resolve");

        assert_eq!(resolved.token, "saved-token");
        assert_eq!(resolved.source, BotTokenSource::Preference);
    }

    #[test]
    // cli[verify auth.bot-token.clear-removes-preference]
    fn clear_bot_token_preference_removes_saved_token() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let app_home = AppHome(temp_dir.path().join("home"));
        save_bot_token_preference(&app_home, "saved-token")
            .expect("bot token preference should save");

        let removed =
            clear_bot_token_preference(&app_home).expect("saved token preference should clear");

        assert!(removed);
        assert!(
            load_bot_token_preference(&app_home)
                .expect("load should succeed")
                .is_none()
        );
    }
}
