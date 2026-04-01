# CLI

This specification covers the current user-facing command line behavior exposed by `teamy-discord-archive`.

## Command Surface

cli[command.surface.core]
The CLI must expose the `bot-token`, `cache`, `home`, `invite`, `live`, `output-dir`, and `sync` commands.

cli[command.surface.bot-token]
The `bot-token` command group must expose the `clear`, `set`, `show-source`, and `validate` subcommands.

cli[command.surface.invite]
The CLI must expose a top-level `invite` command.

cli[command.surface.cache]
The `cache` command group must expose the `show`, `open`, and `clean` subcommands.

cli[command.surface.home]
The `home` command group must expose the `show` and `open` subcommands.

cli[command.surface.output-dir]
The `output-dir` command group must expose the `show`, `open`, and `set` subcommands.

cli[command.surface.live]
The `live` command group must expose the `guild`, `channel`, `thread`, `message`, `attachment`, and `user` subcommands.

cli[command.surface.live-guild]
The `live guild` command group must expose the `list` subcommand.

cli[command.surface.live-channel]
The `live channel` command group must expose the `list` subcommand.

cli[command.surface.live-thread]
The `live thread` command group must expose the `list` subcommand.

cli[command.surface.live-message]
The `live message` command group must expose the `list` subcommand.

cli[command.surface.live-attachment]
The `live attachment` command group must expose the `list` subcommand.

cli[command.surface.live-user]
The `live user` command group must expose the `list` subcommand.

cli[command.surface.sync]
The `sync` command must exist as the resumable archive entrypoint for future Discord archival work.

## Parser Model

cli[parser.args-consistent]
The structured CLI model must serialize to command line arguments consistently for parse-safe values.

cli[parser.roundtrip]
The structured CLI model must roundtrip through argument serialization and parsing for parse-safe values.

## Path Resolution

cli[path.app-home.env-overrides-platform]
If `TEAMY_DISCORD_ARCHIVE_HOME_DIR` is set to a non-empty value, it must take precedence over the platform-derived application home directory.

cli[path.cache.env-overrides-platform]
If `TEAMY_DISCORD_ARCHIVE_CACHE_DIR` is set to a non-empty value, it must take precedence over the platform-derived cache directory.

cli[auth.live-token.env]
If `TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN` is set to a non-empty value, `bot-token`, `invite`, and `live` commands may use it as the Discord bot token.

cli[auth.live-token.command-line-overrides-env]
If `live --token <token>` is provided, it must take precedence over `TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN`.

cli[auth.live-token.preference-fallback]
If neither `live --token <token>` nor `TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN` is provided, `live` commands must use the persisted bot token preference from the application home directory.

cli[auth.invite-token-resolution]
If neither `invite --token <token>` nor `TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN` is provided, the `invite` command must use the persisted bot token preference from the application home directory.

cli[invite.prints-url]
The `invite` command must print the resolved Discord bot invite URL to stdout.

cli[invite.opens-browser-by-default]
The `invite` command must open the resolved Discord bot invite URL in the default browser unless `--no-open` is provided.

cli[auth.bot-token.set-persists-default]
The `bot-token set <token>` command must persist the default Discord bot token in the application home directory.

cli[auth.bot-token.set-env-fallback]
If `bot-token set` is run without a positional token and `TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN` is set to a non-empty value, it must persist the environment token.

cli[auth.bot-token.validate-resolves-effective]
The `bot-token validate` command must validate the effective bot token resolved from command line arguments, environment variables, or persisted configuration.

cli[auth.bot-token.show-source-resolves-effective]
The `bot-token show-source` command must report which token source currently wins without printing the token itself.

cli[auth.bot-token.clear-removes-preference]
The `bot-token clear` command must remove the persisted bot token preference from the application home directory without affecting command-line or environment token sources.

cli[path.output-dir.env-overrides-preference]
If `TEAMY_DISCORD_ARCHIVE_OUTPUT_DIR` is set to a non-empty value, it must take precedence over the persisted output directory preference.

cli[path.output-dir.command-line-overrides-env]
If `sync --output-dir <path>` is provided, it must take precedence over `TEAMY_DISCORD_ARCHIVE_OUTPUT_DIR`.

cli[path.output-dir.show-resolves-effective]
The `output-dir show` command must report the effective resolved output directory.

cli[path.output-dir.open-resolves-effective]
The `output-dir open` command must resolve the same effective output directory that `output-dir show` reports.

cli[path.output-dir.set-persists-default]
The `output-dir set <path>` command must persist the default output directory in the application home directory.

cli[sync.requires-output-dir]
The `sync` command must fail with a clear configuration error if no effective output directory can be resolved from command line arguments, environment variables, or persisted configuration.

cli[sync.requires-token]
The `sync` command must fail with a clear configuration error if no effective Discord bot token can be resolved from command line arguments, environment variables, or persisted configuration.

cli[live.message.before-flag]
The `live message list` command must support a `--before <rfc3339>` flag to query messages before a cursor timestamp.

cli[live.message.target-selection]
The `live message list` and `live attachment list` commands must require exactly one of `--channel-id` or `--thread-id`.