use crate::paths::SyncStateLayout;
use chrono::Utc;
use eyre::Context;
use facet::Facet;
use serenity::all::ChannelType;
use serenity::all::GetMessages;
use serenity::all::GuildChannel;
use serenity::all::GuildId;
use serenity::all::GuildInfo;
use serenity::all::Http;
use serenity::all::Message;
use serenity::all::MessageId;
use sha2::Digest;
use sha2::Sha256;
use std::fmt::Write as _;
use std::path::Path;
use std::path::PathBuf;

const CHECKPOINT_VERSION: u32 = 1;
const MESSAGE_PAGE_LIMIT: u8 = 100;

#[derive(Facet, Clone, Debug, Default, PartialEq, Eq)]
#[facet(rename_all = "kebab-case")]
pub struct SyncCheckpoint {
    pub version: u32,
    pub targets: Vec<SyncTargetCheckpoint>,
}

#[derive(Facet, Clone, Debug, PartialEq, Eq)]
#[facet(rename_all = "kebab-case")]
pub struct SyncTargetCheckpoint {
    pub guild_id: u64,
    pub channel_id: u64,
    pub parent_channel_id: Option<u64>,
    pub newest_message_id: Option<u64>,
    pub oldest_message_id: Option<u64>,
    pub historical_complete: bool,
}

#[derive(Facet, Clone, Debug, PartialEq, Eq)]
#[facet(rename_all = "kebab-case")]
pub struct ArchivedAttachmentIndex {
    pub attachment_id: u64,
    pub sha256: String,
    pub blob_path: String,
    pub filename: String,
    pub size: u32,
    pub content_type: Option<String>,
}

#[derive(Facet, Clone, Debug, PartialEq, Eq)]
#[facet(rename_all = "kebab-case")]
pub struct ArchivedAttachmentReference {
    pub attachment_id: u64,
    pub filename: String,
    pub size: u32,
    pub content_type: Option<String>,
    pub blob_path: String,
    pub sha256: String,
}

#[derive(Facet, Clone, Debug, PartialEq, Eq)]
#[facet(rename_all = "kebab-case")]
pub struct ArchivedMessageRecord {
    pub schema_version: u32,
    pub archived_at: String,
    pub guild_id: u64,
    pub channel_id: u64,
    pub parent_channel_id: Option<u64>,
    pub message_id: u64,
    pub raw_json: String,
    pub attachments: Vec<ArchivedAttachmentReference>,
}

#[derive(Facet, Clone, Debug, Default, PartialEq, Eq)]
#[facet(rename_all = "kebab-case")]
pub struct SyncRunSummary {
    pub output_dir: String,
    pub checkpoint_path: String,
    pub guilds_seen: u64,
    pub channels_seen: u64,
    pub threads_seen: u64,
    pub messages_written: u64,
    pub attachments_downloaded: u64,
}

#[derive(Clone, Debug)]
struct SyncTarget {
    guild_id: GuildId,
    channel: GuildChannel,
    is_thread: bool,
}

impl SyncTarget {
    fn channel_id(&self) -> u64 {
        self.channel.id.get()
    }

    fn parent_channel_id(&self) -> Option<u64> {
        self.channel.parent_id.map(serenity::all::ChannelId::get)
    }

    // archive[impl layout.guild-channel-thread]
    fn root_dir(&self, output_root: &Path) -> PathBuf {
        let guild_root = output_root
            .join("guilds")
            .join(self.guild_id.get().to_string());
        if self.is_thread {
            if let Some(parent_id) = self.parent_channel_id() {
                guild_root
                    .join("channels")
                    .join(parent_id.to_string())
                    .join("threads")
                    .join(self.channel_id().to_string())
            } else {
                guild_root
                    .join("orphan-threads")
                    .join(self.channel_id().to_string())
            }
        } else {
            guild_root
                .join("channels")
                .join(self.channel_id().to_string())
        }
    }

    fn metadata_path(&self, output_root: &Path) -> PathBuf {
        let file_name = if self.is_thread {
            "thread.json"
        } else {
            "channel.json"
        };
        self.root_dir(output_root).join(file_name)
    }

    fn messages_dir(&self, output_root: &Path) -> PathBuf {
        self.root_dir(output_root).join("messages")
    }

    fn checkpoint<'a>(&self, checkpoint: &'a mut SyncCheckpoint) -> &'a mut SyncTargetCheckpoint {
        if let Some(index) = checkpoint.targets.iter().position(|candidate| {
            candidate.guild_id == self.guild_id.get() && candidate.channel_id == self.channel_id()
        }) {
            return &mut checkpoint.targets[index];
        }

        checkpoint.targets.push(SyncTargetCheckpoint {
            guild_id: self.guild_id.get(),
            channel_id: self.channel_id(),
            parent_channel_id: self.parent_channel_id(),
            newest_message_id: None,
            oldest_message_id: None,
            historical_complete: false,
        });
        checkpoint
            .targets
            .last_mut()
            .expect("checkpoint target should exist after push")
    }
}

fn is_syncable_channel_kind(kind: ChannelType) -> bool {
    matches!(
        kind,
        ChannelType::Text
            | ChannelType::News
            | ChannelType::PublicThread
            | ChannelType::PrivateThread
            | ChannelType::NewsThread
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

fn attachments_root(output_root: &Path) -> PathBuf {
    output_root.join("attachments")
}

fn attachment_index_path(output_root: &Path, attachment_id: u64) -> PathBuf {
    attachments_root(output_root)
        .join("by-id")
        .join(format!("{attachment_id}.json"))
}

fn attachment_blob_relative_path(sha256: &str) -> String {
    format!("attachments/blobs/sha256/{}/{sha256}", &sha256[..2])
}

fn attachment_blob_path(output_root: &Path, sha256: &str) -> PathBuf {
    output_root.join(attachment_blob_relative_path(sha256))
}

fn guild_metadata_path(output_root: &Path, guild_id: GuildId) -> PathBuf {
    output_root
        .join("guilds")
        .join(guild_id.get().to_string())
        .join("guild.json")
}

fn load_checkpoint(layout: &SyncStateLayout) -> eyre::Result<SyncCheckpoint> {
    if !layout.checkpoint_path.exists() {
        return Ok(SyncCheckpoint {
            version: CHECKPOINT_VERSION,
            targets: Vec::new(),
        });
    }

    let contents = std::fs::read_to_string(&layout.checkpoint_path).wrap_err_with(|| {
        format!(
            "Failed to read sync checkpoint from {}",
            layout.checkpoint_path.display()
        )
    })?;
    let checkpoint: SyncCheckpoint = facet_json::from_str(&contents).wrap_err_with(|| {
        format!(
            "Failed to parse sync checkpoint from {}",
            layout.checkpoint_path.display()
        )
    })?;
    Ok(checkpoint)
}

fn write_text_file(path: &Path, contents: &str) -> eyre::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(path, contents)
        .wrap_err_with(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

fn write_binary_file(path: &Path, contents: &[u8]) -> eyre::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(path, contents)
        .wrap_err_with(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

fn save_checkpoint(layout: &SyncStateLayout, checkpoint: &SyncCheckpoint) -> eyre::Result<()> {
    let contents =
        facet_json::to_string_pretty(checkpoint).wrap_err("Failed to serialize sync checkpoint")?;
    write_text_file(&layout.checkpoint_path, &contents)
}

fn write_raw_json_file<T>(path: &Path, value: &T) -> eyre::Result<()>
where
    T: serde::Serialize,
{
    let contents = serde_json::to_string_pretty(value)
        .wrap_err_with(|| format!("Failed to serialize raw JSON for {}", path.display()))?;
    write_text_file(path, &contents)
}

fn write_facet_json_file<'facet, T>(path: &Path, value: &T) -> eyre::Result<()>
where
    T: facet::Facet<'facet> + ?Sized,
{
    let contents = facet_json::to_string_pretty(value)
        .wrap_err_with(|| format!("Failed to serialize facet JSON for {}", path.display()))?;
    write_text_file(path, &contents)
}

// archive[impl attachments.deduplicated-store]
fn load_attachment_index(path: &Path) -> eyre::Result<Option<ArchivedAttachmentIndex>> {
    if !path.exists() {
        return Ok(None);
    }

    let contents = std::fs::read_to_string(path)
        .wrap_err_with(|| format!("Failed to read attachment index from {}", path.display()))?;
    let index = facet_json::from_str(&contents)
        .wrap_err_with(|| format!("Failed to parse attachment index from {}", path.display()))?;
    Ok(Some(index))
}

// archive[impl attachments.deduplicated-store]
async fn archive_attachment(
    output_root: &Path,
    attachment: &serenity::all::Attachment,
) -> eyre::Result<(ArchivedAttachmentReference, bool)> {
    let index_path = attachment_index_path(output_root, attachment.id.get());
    if let Some(index) = load_attachment_index(&index_path)? {
        let blob_path = output_root.join(&index.blob_path);
        if blob_path.exists() {
            return Ok((
                ArchivedAttachmentReference {
                    attachment_id: index.attachment_id,
                    filename: index.filename,
                    size: index.size,
                    content_type: index.content_type,
                    blob_path: index.blob_path,
                    sha256: index.sha256,
                },
                false,
            ));
        }
    }

    let bytes = attachment
        .download()
        .await
        .wrap_err_with(|| format!("Failed to download attachment {}", attachment.id.get()))?;
    let sha256 = sha256_hex(&bytes);
    let blob_relative_path = attachment_blob_relative_path(&sha256);
    let blob_path = attachment_blob_path(output_root, &sha256);
    let blob_was_missing = !blob_path.exists();
    if blob_was_missing {
        write_binary_file(&blob_path, &bytes)?;
    }

    let index = ArchivedAttachmentIndex {
        attachment_id: attachment.id.get(),
        sha256: sha256.clone(),
        blob_path: blob_relative_path.clone(),
        filename: attachment.filename.clone(),
        size: attachment.size,
        content_type: attachment.content_type.clone(),
    };
    write_facet_json_file(&index_path, &index)?;

    Ok((
        ArchivedAttachmentReference {
            attachment_id: attachment.id.get(),
            filename: attachment.filename.clone(),
            size: attachment.size,
            content_type: attachment.content_type.clone(),
            blob_path: blob_relative_path,
            sha256,
        },
        blob_was_missing,
    ))
}

// archive[impl goal.lossless-raw-payloads]
async fn archive_message(
    output_root: &Path,
    target: &SyncTarget,
    message: &Message,
) -> eyre::Result<(ArchivedMessageRecord, u64)> {
    let raw_json = serde_json::to_string_pretty(message)
        .wrap_err_with(|| format!("Failed to serialize raw message {}", message.id.get()))?;
    let mut archived_attachments = Vec::new();
    let mut downloaded_count = 0;
    for attachment in &message.attachments {
        let (archived_attachment, downloaded) = archive_attachment(output_root, attachment).await?;
        archived_attachments.push(archived_attachment);
        if downloaded {
            downloaded_count += 1;
        }
    }

    Ok((
        ArchivedMessageRecord {
            schema_version: 1,
            archived_at: Utc::now().to_rfc3339(),
            guild_id: target.guild_id.get(),
            channel_id: target.channel_id(),
            parent_channel_id: target.parent_channel_id(),
            message_id: message.id.get(),
            raw_json,
            attachments: archived_attachments,
        },
        downloaded_count,
    ))
}

fn newest_message_id(messages: &[Message]) -> Option<u64> {
    messages.iter().map(|message| message.id.get()).max()
}

fn oldest_message_id(messages: &[Message]) -> Option<u64> {
    messages.iter().map(|message| message.id.get()).min()
}

// archive[impl sync.writes-output-files]
async fn write_message_page(
    output_root: &Path,
    target: &SyncTarget,
    messages: &[Message],
) -> eyre::Result<(u64, u64)> {
    let messages_dir = target.messages_dir(output_root);
    std::fs::create_dir_all(&messages_dir)
        .wrap_err_with(|| format!("Failed to create {}", messages_dir.display()))?;

    let mut messages_written = 0;
    let mut attachments_downloaded = 0;
    for message in messages {
        let (record, downloaded_count) = archive_message(output_root, target, message).await?;
        let message_path = messages_dir.join(format!("{}.json", message.id.get()));
        write_facet_json_file(&message_path, &record)?;
        messages_written += 1;
        attachments_downloaded += downloaded_count;
    }

    Ok((messages_written, attachments_downloaded))
}

async fn fetch_messages_before(
    http: &Http,
    channel_id: serenity::all::ChannelId,
    before: Option<u64>,
) -> eyre::Result<Vec<Message>> {
    let mut builder = GetMessages::new().limit(MESSAGE_PAGE_LIMIT);
    if let Some(before) = before {
        builder = builder.before(MessageId::new(before));
    }
    channel_id
        .messages(http, builder)
        .await
        .wrap_err_with(|| format!("Failed to list messages for channel {}", channel_id.get()))
}

async fn fetch_messages_after(
    http: &Http,
    channel_id: serenity::all::ChannelId,
    after: u64,
) -> eyre::Result<Vec<Message>> {
    let builder = GetMessages::new()
        .limit(MESSAGE_PAGE_LIMIT)
        .after(MessageId::new(after));
    channel_id
        .messages(http, builder)
        .await
        .wrap_err_with(|| format!("Failed to list messages for channel {}", channel_id.get()))
}

// archive[impl sync.resume-from-checkpoint]
async fn sync_newer_messages(
    http: &Http,
    output_root: &Path,
    layout: &SyncStateLayout,
    checkpoint: &mut SyncCheckpoint,
    target: &SyncTarget,
) -> eyre::Result<(u64, u64)> {
    let mut messages_written = 0;
    let mut attachments_downloaded = 0;

    loop {
        let after = {
            let state = target.checkpoint(checkpoint);
            state.newest_message_id
        };
        let Some(after) = after else {
            break;
        };

        let messages = fetch_messages_after(http, target.channel.id, after).await?;
        if messages.is_empty() {
            break;
        }

        let (page_messages_written, page_attachments_downloaded) =
            write_message_page(output_root, target, &messages).await?;
        messages_written += page_messages_written;
        attachments_downloaded += page_attachments_downloaded;

        {
            let state = target.checkpoint(checkpoint);
            state.newest_message_id = newest_message_id(&messages).or(state.newest_message_id);
            state.oldest_message_id = match (state.oldest_message_id, oldest_message_id(&messages))
            {
                (Some(existing), Some(page_oldest)) => Some(existing.min(page_oldest)),
                (None, page_oldest) => page_oldest,
                (existing, None) => existing,
            };
        };
        save_checkpoint(layout, checkpoint)?;

        if messages.len() < usize::from(MESSAGE_PAGE_LIMIT) {
            break;
        }
    }

    Ok((messages_written, attachments_downloaded))
}

// archive[impl sync.resume-from-checkpoint]
async fn sync_historical_messages(
    http: &Http,
    output_root: &Path,
    layout: &SyncStateLayout,
    checkpoint: &mut SyncCheckpoint,
    target: &SyncTarget,
) -> eyre::Result<(u64, u64)> {
    let mut messages_written = 0;
    let mut attachments_downloaded = 0;

    loop {
        let (historical_complete, before) = {
            let state = target.checkpoint(checkpoint);
            (state.historical_complete, state.oldest_message_id)
        };
        if historical_complete {
            break;
        }

        let messages = fetch_messages_before(http, target.channel.id, before).await?;
        if messages.is_empty() {
            target.checkpoint(checkpoint).historical_complete = true;
            save_checkpoint(layout, checkpoint)?;
            break;
        }

        let (page_messages_written, page_attachments_downloaded) =
            write_message_page(output_root, target, &messages).await?;
        messages_written += page_messages_written;
        attachments_downloaded += page_attachments_downloaded;

        {
            let state = target.checkpoint(checkpoint);
            state.newest_message_id = match (state.newest_message_id, newest_message_id(&messages))
            {
                (Some(existing), Some(page_newest)) => Some(existing.max(page_newest)),
                (None, page_newest) => page_newest,
                (existing, None) => existing,
            };
            state.oldest_message_id = match (state.oldest_message_id, oldest_message_id(&messages))
            {
                (Some(existing), Some(page_oldest)) => Some(existing.min(page_oldest)),
                (None, page_oldest) => page_oldest,
                (existing, None) => existing,
            };
        };
        save_checkpoint(layout, checkpoint)?;
    }

    Ok((messages_written, attachments_downloaded))
}

async fn sync_target(
    http: &Http,
    output_root: &Path,
    layout: &SyncStateLayout,
    checkpoint: &mut SyncCheckpoint,
    target: &SyncTarget,
) -> eyre::Result<(u64, u64)> {
    write_raw_json_file(&target.metadata_path(output_root), &target.channel)?;
    let (new_messages_written, new_attachments_downloaded) =
        sync_newer_messages(http, output_root, layout, checkpoint, target).await?;
    let (historical_messages_written, historical_attachments_downloaded) =
        sync_historical_messages(http, output_root, layout, checkpoint, target).await?;
    Ok((
        new_messages_written + historical_messages_written,
        new_attachments_downloaded + historical_attachments_downloaded,
    ))
}

// archive[impl sync.writes-output-files]
async fn sync_guild(
    http: &Http,
    output_root: &Path,
    layout: &SyncStateLayout,
    checkpoint: &mut SyncCheckpoint,
    guild: &GuildInfo,
    summary: &mut SyncRunSummary,
) -> eyre::Result<()> {
    write_raw_json_file(&guild_metadata_path(output_root, guild.id), guild)?;
    let channels = http
        .get_channels(guild.id)
        .await
        .wrap_err_with(|| format!("Failed to list channels for guild {}", guild.id.get()))?;

    for channel in &channels {
        write_raw_json_file(
            &SyncTarget {
                guild_id: guild.id,
                channel: channel.clone(),
                is_thread: false,
            }
            .metadata_path(output_root),
            channel,
        )?;
    }

    let channel_targets = channels
        .into_iter()
        .filter(|channel| is_syncable_channel_kind(channel.kind))
        .map(|channel| SyncTarget {
            guild_id: guild.id,
            channel,
            is_thread: false,
        })
        .collect::<Vec<_>>();
    summary.channels_seen += u64::try_from(channel_targets.len()).unwrap_or(u64::MAX);

    for target in channel_targets {
        let (messages_written, attachments_downloaded) =
            sync_target(http, output_root, layout, checkpoint, &target).await?;
        summary.messages_written += messages_written;
        summary.attachments_downloaded += attachments_downloaded;
    }

    let threads = http
        .get_guild_active_threads(guild.id)
        .await
        .wrap_err_with(|| format!("Failed to list active threads for guild {}", guild.id.get()))?;
    summary.threads_seen += u64::try_from(threads.threads.len()).unwrap_or(u64::MAX);
    for thread in threads.threads {
        let target = SyncTarget {
            guild_id: guild.id,
            channel: thread,
            is_thread: true,
        };
        let (messages_written, attachments_downloaded) =
            sync_target(http, output_root, layout, checkpoint, &target).await?;
        summary.messages_written += messages_written;
        summary.attachments_downloaded += attachments_downloaded;
    }

    Ok(())
}

/// # Errors
///
/// This function will return an error if the Discord API calls fail or if archive data cannot be written.
// archive[impl goal.resumable-sync-entrypoint]
// archive[impl goal.local-filesystem-output]
pub async fn run_sync(
    output_root: &Path,
    layout: &SyncStateLayout,
    token: &str,
) -> eyre::Result<SyncRunSummary> {
    let http = Http::new(token);
    let mut checkpoint = load_checkpoint(layout)?;
    if checkpoint.version == 0 {
        checkpoint.version = CHECKPOINT_VERSION;
    }

    let guilds = crate::discord::live::list_guilds(&http).await?;
    let mut summary = SyncRunSummary {
        output_dir: output_root.display().to_string(),
        checkpoint_path: layout.checkpoint_path.display().to_string(),
        guilds_seen: u64::try_from(guilds.len()).unwrap_or(u64::MAX),
        channels_seen: 0,
        threads_seen: 0,
        messages_written: 0,
        attachments_downloaded: 0,
    };

    for guild in guilds {
        sync_guild(
            &http,
            output_root,
            layout,
            &mut checkpoint,
            &guild,
            &mut summary,
        )
        .await?;
        save_checkpoint(layout, &checkpoint)?;
    }

    save_checkpoint(layout, &checkpoint)?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::ArchivedAttachmentIndex;
    use super::CHECKPOINT_VERSION;
    use super::SyncCheckpoint;
    use super::SyncTarget;
    use super::SyncTargetCheckpoint;
    use super::attachment_blob_relative_path;
    use super::attachment_index_path;
    use super::attachments_root;
    use super::load_attachment_index;
    use super::load_checkpoint;
    use super::save_checkpoint;
    use crate::paths::CacheHome;
    use crate::paths::ensure_sync_state_layout;
    use serenity::all::ChannelId;
    use serenity::all::ChannelType;
    use serenity::all::GuildChannel;
    use serenity::all::GuildId;
    use tempfile::tempdir;

    #[test]
    // archive[verify attachments.deduplicated-store]
    fn attachment_blob_relative_path_uses_hash_prefix_partitioning() {
        let sha256 = "abcdef0123456789";
        assert_eq!(
            attachment_blob_relative_path(sha256),
            "attachments/blobs/sha256/ab/abcdef0123456789"
        );
    }

    #[test]
    // archive[verify sync.resume-from-checkpoint]
    fn checkpoint_roundtrips_through_facet_json() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let cache_home = CacheHome(temp_dir.path().join("cache"));
        let layout =
            ensure_sync_state_layout(&cache_home, temp_dir.path().join("output").as_path())
                .expect("sync state layout should exist");

        let checkpoint = SyncCheckpoint {
            version: CHECKPOINT_VERSION,
            targets: vec![SyncTargetCheckpoint {
                guild_id: 1,
                channel_id: 2,
                parent_channel_id: Some(3),
                newest_message_id: Some(4),
                oldest_message_id: Some(5),
                historical_complete: true,
            }],
        };

        save_checkpoint(&layout, &checkpoint).expect("checkpoint should save");
        let loaded = load_checkpoint(&layout).expect("checkpoint should load");
        assert_eq!(loaded, checkpoint);
    }

    #[test]
    // archive[verify attachments.deduplicated-store]
    fn attachment_index_roundtrips_from_disk() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let output_root = temp_dir.path().join("output");
        let index_path = attachment_index_path(&output_root, 42);
        let index = ArchivedAttachmentIndex {
            attachment_id: 42,
            sha256: "abc123".to_owned(),
            blob_path: "attachments/blobs/sha256/ab/abc123".to_owned(),
            filename: "file.txt".to_owned(),
            size: 12,
            content_type: Some("text/plain".to_owned()),
        };
        let json = facet_json::to_string_pretty(&index).expect("index should serialize");
        std::fs::create_dir_all(index_path.parent().expect("index parent should exist"))
            .expect("index parent should be created");
        std::fs::write(&index_path, json).expect("index should write");

        let loaded = load_attachment_index(&index_path)
            .expect("index should load")
            .expect("index should exist");
        assert_eq!(loaded, index);
        assert_eq!(
            attachments_root(&output_root),
            output_root.join("attachments")
        );
    }

    #[test]
    // archive[verify layout.guild-channel-thread]
    fn thread_target_is_nested_under_parent_channel() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let output_root = temp_dir.path().join("output");
        let channel: GuildChannel = serde_json::from_value(serde_json::json!({
            "id": "11",
            "type": 11,
            "guild_id": "99",
            "name": "thread-name",
            "position": 0,
            "permission_overwrites": [],
            "nsfw": false,
            "parent_id": "10"
        }))
        .expect("guild channel should deserialize");

        let target = SyncTarget {
            guild_id: GuildId::new(99),
            channel,
            is_thread: true,
        };

        assert_eq!(
            target.root_dir(&output_root),
            output_root
                .join("guilds")
                .join("99")
                .join("channels")
                .join("10")
                .join("threads")
                .join("11")
        );
        assert_eq!(target.channel.kind, ChannelType::PublicThread);
        assert_eq!(target.channel.id, ChannelId::new(11));
    }
}
