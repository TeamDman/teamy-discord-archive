# Archive Goals

This specification captures the product-specific archival goals for `teamy-discord-archive`.

archive[goal.lossless-raw-payloads]
The archiver must preserve raw Discord message payloads losslessly so that later transformations remain additive rather than destructive.

archive[goal.local-filesystem-output]
The archiver must treat the configured output directory as a filesystem target without requiring cloud-specific persistence knowledge.

archive[goal.resumable-sync-entrypoint]
The product must center the historical archival workflow around a resumable `sync` command.

archive[sync.writes-output-files]
The `sync` command must write archived guild, channel, thread, message, and attachment data under the configured output directory.

archive[sync.resume-from-checkpoint]
The `sync` command must persist per-target cursors so that interrupted runs can resume rather than restarting from scratch.

archive[sync.progress.structured-logging]
The `sync` command must emit structured tracing progress updates suitable for live monitoring and Tracy inspection, including resume-aware target state and current sync position.

archive[sync.progress.estimated-telemetry]
The `sync` command must report estimated progress telemetry during archival work, including messages-per-second, bytes processed, estimated remaining messages and bytes, and ETA values.

archive[goal.live-api-probing]
The product must provide a live-query command surface for inspecting Discord API behavior separately from archived data queries.

archive[layout.guild-channel-thread]
The export layout must organize data under guilds, channels, and nested threads so forum-style channels and ordinary channels share a consistent parent-child structure.

archive[state.checkpoints-in-cache]
Resumable operational state must live under the application cache directory rather than inside the export root.

archive[state.per-output-root-isolated]
Each output root synchronized by the `sync` command must map to its own isolated resumable state directory under the application cache directory.

archive[cursor.before-datetime]
History-oriented live message queries should support a backward cursor expressed as a before-datetime value.

archive[attachments.deduplicated-store]
Archived attachments should be stored in a deduplicated shared store under the output root and referenced by exported message metadata.