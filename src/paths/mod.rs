mod app_home;
mod bot_token;
mod cache;
mod output_dir;
mod sync_state;

pub use app_home::*;
pub use bot_token::*;
pub use cache::*;
pub use output_dir::*;
pub use sync_state::*;

pub const APP_HOME_ENV_VAR: &str = "TEAMY_DISCORD_ARCHIVE_HOME_DIR";
pub const APP_HOME_DIR_NAME: &str = "teamy-discord-archive";

pub const APP_CACHE_ENV_VAR: &str = "TEAMY_DISCORD_ARCHIVE_CACHE_DIR";
pub const APP_CACHE_DIR_NAME: &str = "teamy-discord-archive";

pub const BOT_TOKEN_ENV_VAR: &str = "TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN";

pub const OUTPUT_DIR_ENV_VAR: &str = "TEAMY_DISCORD_ARCHIVE_OUTPUT_DIR";
