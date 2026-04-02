- Sync guild roles (colour, priority)
- Sync guild member roles
- If the bot is only in a single guild, automatically infer the --guild-id param

- the sync members command should have logging that indicates total items, current progress, items remaining, throughput, bytes per second, bytes remaining, predicted time remaining
- the sync members command should have resume/skip logic to skip fetching members that were fetched in the last `--ago {humantime}` (default 1 hour)