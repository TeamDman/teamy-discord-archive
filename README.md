# Teamy Discord Archive

[![crates.io](https://img.shields.io/crates/v/teamy-discord-archive.svg)](https://crates.io/crates/teamy-discord-archive)
[![license](https://img.shields.io/crates/l/teamy-discord-archive.svg)](https://crates.io/crates/teamy-discord-archive)

![Teamy Rust CLI media demo](resources/main.png)

Discord archival CLI for exporting guild content to a local filesystem target.

## Current Focus

The current implementation focuses on the first local-first pieces of the archiver:

- `figue` + `facet` based argument parsing
- `--help` and `--version` support, including git revision in version output
- structured logging to stderr with optional NDJSON log files
- app home and cache directory resolution
- persisted Discord bot token preferences with command-line and environment overrides
- a top-level invite command that prints and optionally opens the bot invite URL
- read-only live Discord API inspection commands powered by serenity HTTP
- persisted output directory preferences with environment-variable override
- an initial `sync` command scaffold that resolves and prepares the effective output target

## Quick Start

```powershell
cargo run -- bot-token set
```

Validate the effective token and show the authenticated bot user:

```powershell
cargo run -- bot-token validate
```

Show which source currently wins without printing the token:

```powershell
cargo run -- bot-token show-source
```

Remove the saved token from app-home:

```powershell
cargo run -- bot-token clear
```

Print the bot invite URL and open it in the default browser:

```powershell
cargo run -- invite
```

```powershell
cargo run -- output-dir set C:\archive\discord
```

Resolve the effective output directory:

```powershell
cargo run -- output-dir show
```

Run the sync scaffold against the configured destination:

```powershell
cargo run -- sync
```

List guilds visible to the bot through the live API:

```powershell
$env:TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN = "..."
cargo run -- live guild list
```

## Example Usage

Inspect the generated CLI surface:

```powershell
cargo run -- --help
```

Show the resolved application home directory:

```powershell
cargo run -- home show
```

Show the resolved cache directory:

```powershell
cargo run -- cache show
```

Persist the current environment token into app-home:

```powershell
$env:TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN = "..."
cargo run -- bot-token set
```

Validate an explicit token without saving it:

```powershell
cargo run -- bot-token validate --token "..."
```

Inspect whether the effective token is coming from `--token`, the environment, or the saved preference:

```powershell
cargo run -- bot-token show-source
```

Remove the saved token while leaving the environment variable untouched:

```powershell
cargo run -- bot-token clear
```

Print the bot invite URL without opening the browser:

```powershell
cargo run -- invite --no-open
```

Override the persisted output directory with an environment variable:

```powershell
$env:TEAMY_DISCORD_ARCHIVE_OUTPUT_DIR = "C:\Users\TeamD\rclone-mounts\teamy-discord-archive"
cargo run -- output-dir show
```

Write structured logs to disk while still logging to stderr:

```powershell
cargo run -- --log-file .\logs home show
```

List channels in a guild:

```powershell
cargo run -- live channel list --guild-id 123456789012345678
```

List active threads in a guild:

```powershell
cargo run -- live thread list --guild-id 123456789012345678
```

List users in a guild:

```powershell
cargo run -- live user list --guild-id 123456789012345678 --limit 200
```

List messages in a thread before a timestamp:

```powershell
cargo run -- live message list --thread-id 123456789012345678 --before 2026-04-01T00:00:00Z --limit 50
```

## Environment Variables

The CLI currently recognizes these environment variables:

- `TEAMY_DISCORD_ARCHIVE_HOME_DIR`: overrides the resolved application home directory
- `TEAMY_DISCORD_ARCHIVE_CACHE_DIR`: overrides the resolved cache directory
- `TEAMY_DISCORD_ARCHIVE_DISCORD_BOT_TOKEN`: supplies the Discord bot token for `bot-token`, `invite`, and `live` commands
- `TEAMY_DISCORD_ARCHIVE_OUTPUT_DIR`: overrides the persisted output directory preference
- `RUST_LOG`: provides a tracing filter when `--log-filter` is not supplied

## Quality Gate

Run the standard validation flow with:

```powershell
./check-all.ps1
```

That script runs formatting, clippy, build, tests, and local tracey validation.

For Tracy profiling, run:

```powershell
./run-tracing.ps1 output-dir show
```

## Repository Layout

```text
. # Some files omitted
├── .config/tracey/config.styx # Local tracey specification wiring
├── build.rs # Adds exe resources and embeds git revision
├── Cargo.toml # Package metadata, dependencies, lint policy
├── check-all.ps1 # Formatting, linting, build, tests, tracey validation
├── docs/spec # Human-readable requirements for the repository, CLI, and archive goals
├── resources # Windows resources used by build.rs
├── src # Rust source code
├── tests # CLI roundtrip fuzz tests
└── update.ps1 # Convenience install helper
```
