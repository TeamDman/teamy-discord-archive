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
use std::time::Instant;

const CHECKPOINT_VERSION: u32 = 1;
const MESSAGE_PAGE_LIMIT: u8 = 100;
const DISCORD_EPOCH_MILLIS: i64 = 1_420_070_400_000;
const DISCORD_MISSING_ACCESS_ERROR_CODE: isize = 50_001;
const FALLBACK_ESTIMATED_MESSAGES_PER_TARGET: u64 = 400;
const FALLBACK_ESTIMATED_BYTES_PER_MESSAGE: u64 = 2048;
const MIN_ESTIMATION_SAMPLE_MESSAGES: u64 = 25;

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
    pub archived_message_count: Option<u64>,
    pub archived_byte_count: Option<u64>,
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
    pub resumed_targets: u64,
    pub messages_written: u64,
    pub attachments_downloaded: u64,
    pub bytes_processed: u64,
}

#[derive(Clone, Debug)]
struct SyncTarget {
    guild_id: GuildId,
    guild_name: String,
    channel: GuildChannel,
    is_thread: bool,
    parent_channel_name: Option<String>,
}

impl SyncTarget {
    fn channel_id(&self) -> u64 {
        self.channel.id.get()
    }

    fn parent_channel_id(&self) -> Option<u64> {
        self.channel.parent_id.map(serenity::all::ChannelId::get)
    }

    fn guild_name(&self) -> &str {
        &self.guild_name
    }

    fn channel_name(&self) -> &str {
        if self.is_thread {
            self.parent_channel_name.as_deref().unwrap_or("")
        } else {
            &self.channel.name
        }
    }

    fn thread_name(&self) -> &str {
        if self.is_thread {
            &self.channel.name
        } else {
            ""
        }
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
            archived_message_count: Some(0),
            archived_byte_count: Some(0),
        });
        checkpoint
            .targets
            .last_mut()
            .expect("checkpoint target should exist after push")
    }
}

#[derive(Clone, Debug)]
struct SyncGuildPlan {
    guild: GuildInfo,
    channels: Vec<GuildChannel>,
    channel_targets: Vec<SyncTarget>,
    thread_targets: Vec<SyncTarget>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SyncWorkDelta {
    messages_written: u64,
    attachments_downloaded: u64,
    bytes_processed: u64,
}

impl SyncWorkDelta {
    fn add_assign(&mut self, other: Self) {
        self.messages_written = self.messages_written.saturating_add(other.messages_written);
        self.attachments_downloaded = self
            .attachments_downloaded
            .saturating_add(other.attachments_downloaded);
        self.bytes_processed = self.bytes_processed.saturating_add(other.bytes_processed);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SyncProgressMetrics {
    targets_total: u64,
    targets_completed: u64,
    resumed_targets: u64,
    messages_processed: u64,
    bytes_processed: u64,
    estimated_messages_total: u64,
    estimated_messages_remaining: u64,
    estimated_bytes_total: u64,
    estimated_bytes_remaining: u64,
    message_rate_per_sec: u64,
    bytes_rate_per_sec: u64,
    progress_percent: u64,
    eta_seconds: u64,
    eta_known: bool,
}

#[derive(Debug)]
struct SyncProgressTracker {
    started_at: Instant,
    targets_total: u64,
    resumed_targets: u64,
    targets_completed: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SyncTargetOutcome {
    Completed(SyncWorkDelta),
    SkippedMissingAccess(SyncWorkDelta),
}

impl SyncProgressTracker {
    fn new(targets: &[SyncTarget], checkpoint: &SyncCheckpoint) -> Self {
        let resumed_targets = targets
            .iter()
            .filter(|target| {
                checkpoint_state(checkpoint, target).is_some_and(target_has_resume_state)
            })
            .count();

        Self {
            started_at: Instant::now(),
            targets_total: u64::try_from(targets.len()).unwrap_or(u64::MAX),
            resumed_targets: u64::try_from(resumed_targets).unwrap_or(u64::MAX),
            targets_completed: 0,
        }
    }

    fn mark_target_complete(&mut self) {
        self.targets_completed = self.targets_completed.saturating_add(1);
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

fn snowflake_timestamp_millis(snowflake_id: u64) -> i64 {
    i64::try_from(snowflake_id >> 22)
        .unwrap_or(i64::MAX)
        .saturating_add(DISCORD_EPOCH_MILLIS)
}

fn checkpoint_state<'a>(
    checkpoint: &'a SyncCheckpoint,
    target: &SyncTarget,
) -> Option<&'a SyncTargetCheckpoint> {
    checkpoint.targets.iter().find(|candidate| {
        candidate.guild_id == target.guild_id.get() && candidate.channel_id == target.channel_id()
    })
}

fn checkpoint_archived_messages(state: Option<&SyncTargetCheckpoint>) -> u64 {
    state
        .and_then(|candidate| candidate.archived_message_count)
        .unwrap_or(0)
}

fn checkpoint_archived_bytes(state: Option<&SyncTargetCheckpoint>) -> u64 {
    state
        .and_then(|candidate| candidate.archived_byte_count)
        .unwrap_or(0)
}

fn discord_error_code(error: &serenity::Error) -> Option<isize> {
    match error {
        serenity::Error::Http(serenity::http::HttpError::UnsuccessfulRequest(response)) => {
            Some(response.error.code)
        }
        _ => None,
    }
}

fn is_missing_access_error_code(error_code: Option<isize>) -> bool {
    error_code == Some(DISCORD_MISSING_ACCESS_ERROR_CODE)
}

fn is_missing_access_error(error: &serenity::Error) -> bool {
    is_missing_access_error_code(discord_error_code(error))
}

fn target_has_resume_state(state: &SyncTargetCheckpoint) -> bool {
    state.newest_message_id.is_some()
        || state.oldest_message_id.is_some()
        || state.historical_complete
        || state.archived_message_count.unwrap_or(0) > 0
        || state.archived_byte_count.unwrap_or(0) > 0
}

fn estimate_target_total_messages(
    target: &SyncTarget,
    state: Option<&SyncTargetCheckpoint>,
    fallback_messages_per_target: u64,
) -> u64 {
    let processed_messages = checkpoint_archived_messages(state);
    let Some(state) = state else {
        return fallback_messages_per_target.max(1);
    };

    if state.historical_complete {
        return processed_messages.max(1);
    }

    let Some(newest_id) = state.newest_message_id else {
        return processed_messages.saturating_add(fallback_messages_per_target.max(1));
    };
    let Some(oldest_id) = state.oldest_message_id else {
        return processed_messages.saturating_add(fallback_messages_per_target.max(1));
    };

    if processed_messages < MIN_ESTIMATION_SAMPLE_MESSAGES {
        return processed_messages.saturating_add(fallback_messages_per_target.max(1));
    }

    let newest_ts = snowflake_timestamp_millis(newest_id);
    let oldest_ts = snowflake_timestamp_millis(oldest_id);
    let channel_start_ts = snowflake_timestamp_millis(target.channel_id());
    let observed_span = newest_ts.saturating_sub(oldest_ts);
    if observed_span <= 0 {
        return processed_messages.saturating_add(fallback_messages_per_target.max(1));
    }

    let remaining_span =
        u64::try_from(oldest_ts.saturating_sub(channel_start_ts).max(0)).unwrap_or(u64::MAX);
    let observed_span = u64::try_from(observed_span).unwrap_or(1);
    let estimated_historical_remaining = u64::try_from(
        u128::from(processed_messages).saturating_mul(u128::from(remaining_span))
            / u128::from(observed_span),
    )
    .unwrap_or(u64::MAX);

    processed_messages
        .saturating_add(estimated_historical_remaining)
        .max(processed_messages.saturating_add(1))
}

fn estimate_target_total_bytes(
    processed_messages: u64,
    processed_bytes: u64,
    estimated_total_messages: u64,
    fallback_bytes_per_message: u64,
) -> u64 {
    if estimated_total_messages == 0 {
        return 0;
    }

    if processed_messages == 0 || processed_bytes == 0 {
        return estimated_total_messages.saturating_mul(fallback_bytes_per_message.max(1));
    }

    let average_bytes_per_message = (processed_bytes / processed_messages).max(1);
    estimated_total_messages.saturating_mul(average_bytes_per_message)
}

fn overall_progress_metrics(
    targets: &[SyncTarget],
    checkpoint: &SyncCheckpoint,
    tracker: &SyncProgressTracker,
) -> SyncProgressMetrics {
    let observed_messages = targets
        .iter()
        .map(|target| checkpoint_archived_messages(checkpoint_state(checkpoint, target)))
        .sum::<u64>();
    let observed_bytes = targets
        .iter()
        .map(|target| checkpoint_archived_bytes(checkpoint_state(checkpoint, target)))
        .sum::<u64>();
    let started_target_count = targets
        .iter()
        .filter(|target| checkpoint_state(checkpoint, target).is_some())
        .count();

    let fallback_messages_per_target = if started_target_count == 0 {
        FALLBACK_ESTIMATED_MESSAGES_PER_TARGET
    } else {
        (observed_messages / u64::try_from(started_target_count).unwrap_or(1))
            .max(FALLBACK_ESTIMATED_MESSAGES_PER_TARGET)
    };
    let fallback_bytes_per_message = if observed_messages == 0 {
        FALLBACK_ESTIMATED_BYTES_PER_MESSAGE
    } else {
        (observed_bytes / observed_messages).max(FALLBACK_ESTIMATED_BYTES_PER_MESSAGE)
    };

    let mut estimated_messages_total = 0_u64;
    let mut estimated_bytes_total = 0_u64;
    for target in targets {
        let state = checkpoint_state(checkpoint, target);
        let processed_messages = checkpoint_archived_messages(state);
        let processed_bytes = checkpoint_archived_bytes(state);
        let estimated_total_messages =
            estimate_target_total_messages(target, state, fallback_messages_per_target);
        estimated_messages_total =
            estimated_messages_total.saturating_add(estimated_total_messages);
        estimated_bytes_total = estimated_bytes_total.saturating_add(estimate_target_total_bytes(
            processed_messages,
            processed_bytes,
            estimated_total_messages,
            fallback_bytes_per_message,
        ));
    }

    let estimated_messages_remaining = estimated_messages_total.saturating_sub(observed_messages);
    let estimated_bytes_remaining = estimated_bytes_total.saturating_sub(observed_bytes);
    let elapsed_seconds = tracker.started_at.elapsed().as_secs().max(1);
    let message_rate_per_sec = observed_messages / elapsed_seconds;
    let bytes_rate_per_sec = observed_bytes / elapsed_seconds;
    let eta_known = message_rate_per_sec > 0;
    let eta_seconds = if eta_known {
        estimated_messages_remaining / message_rate_per_sec.max(1)
    } else {
        0
    };
    let progress_percent = if estimated_messages_total > 0 {
        u64::try_from(
            u128::from(observed_messages).saturating_mul(100)
                / u128::from(estimated_messages_total),
        )
        .unwrap_or(100)
    } else if tracker.targets_total > 0 {
        u64::try_from(
            u128::from(tracker.targets_completed).saturating_mul(100)
                / u128::from(tracker.targets_total),
        )
        .unwrap_or(100)
    } else {
        100
    };

    SyncProgressMetrics {
        targets_total: tracker.targets_total,
        targets_completed: tracker.targets_completed,
        resumed_targets: tracker.resumed_targets,
        messages_processed: observed_messages,
        bytes_processed: observed_bytes,
        estimated_messages_total,
        estimated_messages_remaining,
        estimated_bytes_total,
        estimated_bytes_remaining,
        message_rate_per_sec,
        bytes_rate_per_sec,
        progress_percent,
        eta_seconds,
        eta_known,
    }
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

fn write_text_file(path: &Path, contents: &str) -> eyre::Result<u64> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(path, contents)
        .wrap_err_with(|| format!("Failed to write {}", path.display()))?;
    Ok(u64::try_from(contents.len()).unwrap_or(u64::MAX))
}

fn write_binary_file(path: &Path, contents: &[u8]) -> eyre::Result<u64> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(path, contents)
        .wrap_err_with(|| format!("Failed to write {}", path.display()))?;
    Ok(u64::try_from(contents.len()).unwrap_or(u64::MAX))
}

fn save_checkpoint(layout: &SyncStateLayout, checkpoint: &SyncCheckpoint) -> eyre::Result<()> {
    let contents =
        facet_json::to_string_pretty(checkpoint).wrap_err("Failed to serialize sync checkpoint")?;
    let _ = write_text_file(&layout.checkpoint_path, &contents)?;
    Ok(())
}

fn write_raw_json_file<T>(path: &Path, value: &T) -> eyre::Result<u64>
where
    T: serde::Serialize,
{
    let contents = serde_json::to_string_pretty(value)
        .wrap_err_with(|| format!("Failed to serialize raw JSON for {}", path.display()))?;
    write_text_file(path, &contents)
}

fn write_facet_json_file<'facet, T>(path: &Path, value: &T) -> eyre::Result<u64>
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
) -> eyre::Result<(ArchivedAttachmentReference, SyncWorkDelta)> {
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
                SyncWorkDelta::default(),
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
    let mut delta = SyncWorkDelta::default();
    let blob_was_missing = !blob_path.exists();
    if blob_was_missing {
        delta.bytes_processed = delta
            .bytes_processed
            .saturating_add(write_binary_file(&blob_path, &bytes)?);
        delta.attachments_downloaded = 1;
    }

    let index = ArchivedAttachmentIndex {
        attachment_id: attachment.id.get(),
        sha256: sha256.clone(),
        blob_path: blob_relative_path.clone(),
        filename: attachment.filename.clone(),
        size: attachment.size,
        content_type: attachment.content_type.clone(),
    };
    delta.bytes_processed = delta
        .bytes_processed
        .saturating_add(write_facet_json_file(&index_path, &index)?);

    Ok((
        ArchivedAttachmentReference {
            attachment_id: attachment.id.get(),
            filename: attachment.filename.clone(),
            size: attachment.size,
            content_type: attachment.content_type.clone(),
            blob_path: blob_relative_path,
            sha256,
        },
        delta,
    ))
}

// archive[impl goal.lossless-raw-payloads]
async fn archive_message(
    output_root: &Path,
    target: &SyncTarget,
    message: &Message,
) -> eyre::Result<(ArchivedMessageRecord, SyncWorkDelta)> {
    let raw_json = serde_json::to_string_pretty(message)
        .wrap_err_with(|| format!("Failed to serialize raw message {}", message.id.get()))?;
    let mut archived_attachments = Vec::new();
    let mut delta = SyncWorkDelta::default();
    for attachment in &message.attachments {
        let (archived_attachment, attachment_delta) =
            archive_attachment(output_root, attachment).await?;
        archived_attachments.push(archived_attachment);
        delta.add_assign(attachment_delta);
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
        delta,
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
) -> eyre::Result<SyncWorkDelta> {
    let messages_dir = target.messages_dir(output_root);
    std::fs::create_dir_all(&messages_dir)
        .wrap_err_with(|| format!("Failed to create {}", messages_dir.display()))?;

    let mut delta = SyncWorkDelta::default();
    for message in messages {
        let (record, message_delta) = archive_message(output_root, target, message).await?;
        let message_path = messages_dir.join(format!("{}.json", message.id.get()));
        let record_bytes = write_facet_json_file(&message_path, &record)?;
        delta.add_assign(message_delta);
        delta.messages_written = delta.messages_written.saturating_add(1);
        delta.bytes_processed = delta.bytes_processed.saturating_add(record_bytes);
    }

    Ok(delta)
}

async fn fetch_messages_before(
    http: &Http,
    channel_id: serenity::all::ChannelId,
    before: Option<u64>,
) -> serenity::Result<Vec<Message>> {
    let mut builder = GetMessages::new().limit(MESSAGE_PAGE_LIMIT);
    if let Some(before) = before {
        builder = builder.before(MessageId::new(before));
    }
    channel_id.messages(http, builder).await
}

async fn fetch_messages_after(
    http: &Http,
    channel_id: serenity::all::ChannelId,
    after: u64,
) -> serenity::Result<Vec<Message>> {
    let builder = GetMessages::new()
        .limit(MESSAGE_PAGE_LIMIT)
        .after(MessageId::new(after));
    channel_id.messages(http, builder).await
}

// archive[impl sync.progress.structured-logging]
fn log_target_started(
    tracker: &SyncProgressTracker,
    checkpoint: &SyncCheckpoint,
    target: &SyncTarget,
    target_index: usize,
) {
    let state = checkpoint_state(checkpoint, target);
    let resumed = state.is_some_and(target_has_resume_state);
    tracing::info!(
        event = "sync.target.start",
        guild_id = target.guild_id.get(),
        guild_name = target.guild_name(),
        channel_id = target.channel_id(),
        channel_name = target.channel_name(),
        thread_name = target.thread_name(),
        parent_channel_id = target.parent_channel_id(),
        is_thread = target.is_thread,
        target_index = target_index + 1,
        total_targets = tracker.targets_total,
        resumed,
        historical_complete = state.is_some_and(|candidate| candidate.historical_complete),
        checkpoint_messages = checkpoint_archived_messages(state),
        checkpoint_bytes = checkpoint_archived_bytes(state),
        newest_message_id = state
            .and_then(|candidate| candidate.newest_message_id)
            .unwrap_or(0),
        oldest_message_id = state
            .and_then(|candidate| candidate.oldest_message_id)
            .unwrap_or(0),
        "sync target started"
    );
}

fn log_target_skipped(
    target: &SyncTarget,
    target_index: usize,
    phase: &str,
    error: &serenity::Error,
) {
    tracing::warn!(
        event = "sync.target.skipped",
        phase,
        reason = "missing-access",
        guild_id = target.guild_id.get(),
        guild_name = target.guild_name(),
        channel_id = target.channel_id(),
        channel_name = target.channel_name(),
        thread_name = target.thread_name(),
        parent_channel_id = target.parent_channel_id(),
        is_thread = target.is_thread,
        target_index = target_index + 1,
        discord_error_code = discord_error_code(error).unwrap_or_default(),
        error = %error,
        "sync target skipped due to missing access"
    );
}

// archive[impl sync.progress.estimated-telemetry]
fn log_sync_progress(
    tracker: &SyncProgressTracker,
    targets: &[SyncTarget],
    checkpoint: &SyncCheckpoint,
    target: &SyncTarget,
    target_index: usize,
    phase: &str,
    delta: SyncWorkDelta,
) {
    let metrics = overall_progress_metrics(targets, checkpoint, tracker);
    tracing::info!(
        event = "sync.progress",
        phase,
        guild_id = target.guild_id.get(),
        guild_name = target.guild_name(),
        channel_id = target.channel_id(),
        channel_name = target.channel_name(),
        thread_name = target.thread_name(),
        parent_channel_id = target.parent_channel_id(),
        is_thread = target.is_thread,
        target_index = target_index + 1,
        total_targets = metrics.targets_total,
        targets_completed = metrics.targets_completed,
        resumed_targets = metrics.resumed_targets,
        page_messages = delta.messages_written,
        page_attachments_downloaded = delta.attachments_downloaded,
        page_bytes_processed = delta.bytes_processed,
        messages_processed = metrics.messages_processed,
        bytes_processed = metrics.bytes_processed,
        estimated_messages_total = metrics.estimated_messages_total,
        estimated_messages_remaining = metrics.estimated_messages_remaining,
        estimated_bytes_total = metrics.estimated_bytes_total,
        estimated_bytes_remaining = metrics.estimated_bytes_remaining,
        messages_per_second = metrics.message_rate_per_sec,
        bytes_per_second = metrics.bytes_rate_per_sec,
        eta_seconds = metrics.eta_seconds,
        eta_known = metrics.eta_known,
        progress_percent = metrics.progress_percent,
        "sync progress updated"
    );
}

fn log_sync_finished(
    tracker: &SyncProgressTracker,
    targets: &[SyncTarget],
    checkpoint: &SyncCheckpoint,
    summary: &SyncRunSummary,
) {
    let metrics = overall_progress_metrics(targets, checkpoint, tracker);
    tracing::info!(
        event = "sync.complete",
        guilds_seen = summary.guilds_seen,
        channels_seen = summary.channels_seen,
        threads_seen = summary.threads_seen,
        resumed_targets = summary.resumed_targets,
        messages_written = summary.messages_written,
        attachments_downloaded = summary.attachments_downloaded,
        bytes_processed = summary.bytes_processed,
        estimated_messages_total = metrics.estimated_messages_total,
        estimated_bytes_total = metrics.estimated_bytes_total,
        elapsed_seconds = tracker.started_at.elapsed().as_secs(),
        "sync run completed"
    );
}

async fn build_sync_plans(http: &Http, guilds: &[GuildInfo]) -> eyre::Result<Vec<SyncGuildPlan>> {
    let mut plans = Vec::with_capacity(guilds.len());
    for guild in guilds {
        let channels = http
            .get_channels(guild.id)
            .await
            .wrap_err_with(|| format!("Failed to list channels for guild {}", guild.id.get()))?;
        let channel_names = channels
            .iter()
            .map(|channel| (channel.id, channel.name.clone()))
            .collect::<std::collections::HashMap<_, _>>();
        let channel_targets = channels
            .iter()
            .filter(|channel| is_syncable_channel_kind(channel.kind))
            .cloned()
            .map(|channel| SyncTarget {
                guild_id: guild.id,
                guild_name: guild.name.clone(),
                channel,
                is_thread: false,
                parent_channel_name: None,
            })
            .collect::<Vec<_>>();
        let threads = http
            .get_guild_active_threads(guild.id)
            .await
            .wrap_err_with(|| {
                format!("Failed to list active threads for guild {}", guild.id.get())
            })?;
        let thread_targets = threads
            .threads
            .into_iter()
            .map(|channel| SyncTarget {
                guild_id: guild.id,
                guild_name: guild.name.clone(),
                parent_channel_name: channel
                    .parent_id
                    .and_then(|parent_id| channel_names.get(&parent_id).cloned()),
                channel,
                is_thread: true,
            })
            .collect::<Vec<_>>();

        plans.push(SyncGuildPlan {
            guild: guild.clone(),
            channels,
            channel_targets,
            thread_targets,
        });
    }
    Ok(plans)
}

fn flatten_sync_targets(plans: &[SyncGuildPlan]) -> Vec<SyncTarget> {
    let mut targets = Vec::new();
    for plan in plans {
        targets.extend(plan.channel_targets.iter().cloned());
        targets.extend(plan.thread_targets.iter().cloned());
    }
    targets
}

// archive[impl sync.resume-from-checkpoint]
#[expect(
    clippy::too_many_arguments,
    reason = "sync page telemetry needs the current run context"
)]
async fn sync_newer_messages(
    http: &Http,
    output_root: &Path,
    layout: &SyncStateLayout,
    checkpoint: &mut SyncCheckpoint,
    target: &SyncTarget,
    tracker: &SyncProgressTracker,
    targets: &[SyncTarget],
    target_index: usize,
) -> eyre::Result<Option<SyncWorkDelta>> {
    let mut delta = SyncWorkDelta::default();

    loop {
        let after = {
            let state = target.checkpoint(checkpoint);
            state.newest_message_id
        };
        let Some(after) = after else {
            break;
        };

        let messages = match fetch_messages_after(http, target.channel.id, after).await {
            Ok(messages) => messages,
            Err(error) if is_missing_access_error(&error) => {
                log_target_skipped(target, target_index, "newer-fetch", &error);
                return Ok(None);
            }
            Err(error) => {
                return Err(error).wrap_err_with(|| {
                    format!(
                        "Failed to list messages for channel {}",
                        target.channel.id.get()
                    )
                });
            }
        };
        if messages.is_empty() {
            break;
        }

        let page_delta = write_message_page(output_root, target, &messages).await?;
        delta.add_assign(page_delta);

        {
            let state = target.checkpoint(checkpoint);
            state.newest_message_id = newest_message_id(&messages).or(state.newest_message_id);
            state.oldest_message_id = match (state.oldest_message_id, oldest_message_id(&messages))
            {
                (Some(existing), Some(page_oldest)) => Some(existing.min(page_oldest)),
                (None, page_oldest) => page_oldest,
                (existing, None) => existing,
            };
            state.archived_message_count = Some(
                state
                    .archived_message_count
                    .unwrap_or(0)
                    .saturating_add(page_delta.messages_written),
            );
            state.archived_byte_count = Some(
                state
                    .archived_byte_count
                    .unwrap_or(0)
                    .saturating_add(page_delta.bytes_processed),
            );
        };
        save_checkpoint(layout, checkpoint)?;
        log_sync_progress(
            tracker,
            targets,
            checkpoint,
            target,
            target_index,
            "newer-page",
            page_delta,
        );

        if messages.len() < usize::from(MESSAGE_PAGE_LIMIT) {
            break;
        }
    }

    Ok(Some(delta))
}

// archive[impl sync.resume-from-checkpoint]
#[expect(
    clippy::too_many_arguments,
    reason = "sync page telemetry needs the current run context"
)]
async fn sync_historical_messages(
    http: &Http,
    output_root: &Path,
    layout: &SyncStateLayout,
    checkpoint: &mut SyncCheckpoint,
    target: &SyncTarget,
    tracker: &SyncProgressTracker,
    targets: &[SyncTarget],
    target_index: usize,
) -> eyre::Result<Option<SyncWorkDelta>> {
    let mut delta = SyncWorkDelta::default();

    loop {
        let (historical_complete, before) = {
            let state = target.checkpoint(checkpoint);
            (state.historical_complete, state.oldest_message_id)
        };
        if historical_complete {
            break;
        }

        let messages = match fetch_messages_before(http, target.channel.id, before).await {
            Ok(messages) => messages,
            Err(error) if is_missing_access_error(&error) => {
                log_target_skipped(target, target_index, "historical-fetch", &error);
                return Ok(None);
            }
            Err(error) => {
                return Err(error).wrap_err_with(|| {
                    format!(
                        "Failed to list messages for channel {}",
                        target.channel.id.get()
                    )
                });
            }
        };
        if messages.is_empty() {
            target.checkpoint(checkpoint).historical_complete = true;
            save_checkpoint(layout, checkpoint)?;
            break;
        }

        let page_delta = write_message_page(output_root, target, &messages).await?;
        delta.add_assign(page_delta);

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
            state.archived_message_count = Some(
                state
                    .archived_message_count
                    .unwrap_or(0)
                    .saturating_add(page_delta.messages_written),
            );
            state.archived_byte_count = Some(
                state
                    .archived_byte_count
                    .unwrap_or(0)
                    .saturating_add(page_delta.bytes_processed),
            );
        };
        save_checkpoint(layout, checkpoint)?;
        log_sync_progress(
            tracker,
            targets,
            checkpoint,
            target,
            target_index,
            "historical-page",
            page_delta,
        );
    }

    Ok(Some(delta))
}

#[expect(
    clippy::too_many_arguments,
    reason = "target sync needs both resume state and telemetry context"
)]
async fn sync_target(
    http: &Http,
    output_root: &Path,
    layout: &SyncStateLayout,
    checkpoint: &mut SyncCheckpoint,
    target: &SyncTarget,
    tracker: &SyncProgressTracker,
    targets: &[SyncTarget],
    target_index: usize,
) -> eyre::Result<SyncTargetOutcome> {
    let mut delta = SyncWorkDelta::default();
    delta.bytes_processed = delta.bytes_processed.saturating_add(write_raw_json_file(
        &target.metadata_path(output_root),
        &target.channel,
    )?);
    let Some(newer_delta) = sync_newer_messages(
        http,
        output_root,
        layout,
        checkpoint,
        target,
        tracker,
        targets,
        target_index,
    )
    .await?
    else {
        return Ok(SyncTargetOutcome::SkippedMissingAccess(delta));
    };
    delta.add_assign(newer_delta);

    let Some(historical_delta) = sync_historical_messages(
        http,
        output_root,
        layout,
        checkpoint,
        target,
        tracker,
        targets,
        target_index,
    )
    .await?
    else {
        return Ok(SyncTargetOutcome::SkippedMissingAccess(delta));
    };
    delta.add_assign(historical_delta);
    Ok(SyncTargetOutcome::Completed(delta))
}

// archive[impl sync.writes-output-files]
#[expect(
    clippy::too_many_arguments,
    reason = "guild sync coordinates plan data, checkpoint state, and telemetry state"
)]
#[expect(
    clippy::too_many_lines,
    reason = "the per-guild sync flow is clearer when kept in one place"
)]
async fn sync_guild(
    http: &Http,
    output_root: &Path,
    layout: &SyncStateLayout,
    checkpoint: &mut SyncCheckpoint,
    plan: &SyncGuildPlan,
    targets: &[SyncTarget],
    tracker: &mut SyncProgressTracker,
    next_target_index: &mut usize,
    summary: &mut SyncRunSummary,
) -> eyre::Result<()> {
    summary.bytes_processed = summary.bytes_processed.saturating_add(write_raw_json_file(
        &guild_metadata_path(output_root, plan.guild.id),
        &plan.guild,
    )?);

    for channel in &plan.channels {
        summary.bytes_processed = summary.bytes_processed.saturating_add(write_raw_json_file(
            &SyncTarget {
                guild_id: plan.guild.id,
                guild_name: plan.guild.name.clone(),
                channel: channel.clone(),
                is_thread: false,
                parent_channel_name: None,
            }
            .metadata_path(output_root),
            channel,
        )?);
    }

    summary.channels_seen = summary
        .channels_seen
        .saturating_add(u64::try_from(plan.channel_targets.len()).unwrap_or(u64::MAX));
    for target in &plan.channel_targets {
        let target_span = tracing::info_span!(
            "sync_target",
            guild_id = target.guild_id.get(),
            guild_name = target.guild_name(),
            channel_id = target.channel_id(),
            channel_name = target.channel_name(),
            thread_name = target.thread_name(),
            parent_channel_id = target.parent_channel_id(),
            is_thread = target.is_thread
        );
        let _target_guard = target_span.enter();
        log_target_started(tracker, checkpoint, target, *next_target_index);
        let (target_delta, progress_phase) = match sync_target(
            http,
            output_root,
            layout,
            checkpoint,
            target,
            tracker,
            targets,
            *next_target_index,
        )
        .await?
        {
            SyncTargetOutcome::Completed(target_delta) => (target_delta, "target-complete"),
            SyncTargetOutcome::SkippedMissingAccess(target_delta) => {
                (target_delta, "target-skipped")
            }
        };
        summary.messages_written = summary
            .messages_written
            .saturating_add(target_delta.messages_written);
        summary.attachments_downloaded = summary
            .attachments_downloaded
            .saturating_add(target_delta.attachments_downloaded);
        summary.bytes_processed = summary
            .bytes_processed
            .saturating_add(target_delta.bytes_processed);
        log_sync_progress(
            tracker,
            targets,
            checkpoint,
            target,
            *next_target_index,
            progress_phase,
            target_delta,
        );
        tracker.mark_target_complete();
        *next_target_index += 1;
    }

    summary.threads_seen = summary
        .threads_seen
        .saturating_add(u64::try_from(plan.thread_targets.len()).unwrap_or(u64::MAX));
    for target in &plan.thread_targets {
        let target_span = tracing::info_span!(
            "sync_target",
            guild_id = target.guild_id.get(),
            guild_name = target.guild_name(),
            channel_id = target.channel_id(),
            channel_name = target.channel_name(),
            thread_name = target.thread_name(),
            parent_channel_id = target.parent_channel_id(),
            is_thread = target.is_thread
        );
        let _target_guard = target_span.enter();
        log_target_started(tracker, checkpoint, target, *next_target_index);
        let (target_delta, progress_phase) = match sync_target(
            http,
            output_root,
            layout,
            checkpoint,
            target,
            tracker,
            targets,
            *next_target_index,
        )
        .await?
        {
            SyncTargetOutcome::Completed(target_delta) => (target_delta, "target-complete"),
            SyncTargetOutcome::SkippedMissingAccess(target_delta) => {
                (target_delta, "target-skipped")
            }
        };
        summary.messages_written = summary
            .messages_written
            .saturating_add(target_delta.messages_written);
        summary.attachments_downloaded = summary
            .attachments_downloaded
            .saturating_add(target_delta.attachments_downloaded);
        summary.bytes_processed = summary
            .bytes_processed
            .saturating_add(target_delta.bytes_processed);
        log_sync_progress(
            tracker,
            targets,
            checkpoint,
            target,
            *next_target_index,
            progress_phase,
            target_delta,
        );
        tracker.mark_target_complete();
        *next_target_index += 1;
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
    let plans = build_sync_plans(&http, &guilds).await?;
    let targets = flatten_sync_targets(&plans);
    let tracker = &mut SyncProgressTracker::new(&targets, &checkpoint);
    let mut summary = SyncRunSummary {
        output_dir: output_root.display().to_string(),
        checkpoint_path: layout.checkpoint_path.display().to_string(),
        guilds_seen: u64::try_from(guilds.len()).unwrap_or(u64::MAX),
        channels_seen: 0,
        threads_seen: 0,
        resumed_targets: tracker.resumed_targets,
        messages_written: 0,
        attachments_downloaded: 0,
        bytes_processed: 0,
    };

    let run_span = tracing::info_span!(
        "sync_run",
        output_dir = %output_root.display(),
        checkpoint_path = %layout.checkpoint_path.display(),
        total_guilds = summary.guilds_seen,
        total_targets = tracker.targets_total,
        resumed_targets = tracker.resumed_targets
    );
    let _run_guard = run_span.enter();
    tracing::info!(
        event = "sync.start",
        checkpoint_targets = checkpoint.targets.len(),
        total_guilds = summary.guilds_seen,
        total_targets = tracker.targets_total,
        resumed_targets = tracker.resumed_targets,
        "sync run started"
    );

    let mut next_target_index = 0_usize;
    for plan in &plans {
        sync_guild(
            &http,
            output_root,
            layout,
            &mut checkpoint,
            plan,
            &targets,
            tracker,
            &mut next_target_index,
            &mut summary,
        )
        .await?;
        save_checkpoint(layout, &checkpoint)?;
    }

    save_checkpoint(layout, &checkpoint)?;
    log_sync_finished(tracker, &targets, &checkpoint, &summary);
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::ArchivedAttachmentIndex;
    use super::CHECKPOINT_VERSION;
    use super::SyncCheckpoint;
    use super::SyncProgressTracker;
    use super::SyncTarget;
    use super::SyncTargetCheckpoint;
    use super::attachment_blob_relative_path;
    use super::attachment_index_path;
    use super::attachments_root;
    use super::checkpoint_state;
    use super::estimate_target_total_messages;
    use super::is_missing_access_error_code;
    use super::load_attachment_index;
    use super::load_checkpoint;
    use super::overall_progress_metrics;
    use super::save_checkpoint;
    use crate::paths::CacheHome;
    use crate::paths::ensure_sync_state_layout;
    use serenity::all::ChannelId;
    use serenity::all::ChannelType;
    use serenity::all::GuildChannel;
    use serenity::all::GuildId;
    use std::time::Instant;
    use tempfile::tempdir;

    fn test_target(channel_id: u64, guild_id: u64, is_thread: bool) -> SyncTarget {
        let channel_type = if is_thread { 11 } else { 0 };
        let channel: GuildChannel = serde_json::from_value(serde_json::json!({
            "id": channel_id.to_string(),
            "type": channel_type,
            "guild_id": guild_id.to_string(),
            "name": format!("channel-{channel_id}"),
            "position": 0,
            "permission_overwrites": [],
            "nsfw": false,
            "parent_id": serde_json::Value::Null
        }))
        .expect("guild channel should deserialize");

        SyncTarget {
            guild_id: GuildId::new(guild_id),
            guild_name: format!("guild-{guild_id}"),
            channel,
            is_thread,
            parent_channel_name: None,
        }
    }

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
                archived_message_count: Some(6),
                archived_byte_count: Some(7),
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
            guild_name: "guild-99".to_owned(),
            channel,
            is_thread: true,
            parent_channel_name: Some("parent-channel".to_owned()),
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
        assert_eq!(target.guild_name(), "guild-99");
        assert_eq!(target.channel_name(), "parent-channel");
        assert_eq!(target.thread_name(), "thread-name");
    }

    #[test]
    // archive[verify sync.progress.structured-logging]
    fn checkpoint_state_detects_resume_progress_for_target() {
        let target = test_target(11, 99, false);
        let checkpoint = SyncCheckpoint {
            version: CHECKPOINT_VERSION,
            targets: vec![SyncTargetCheckpoint {
                guild_id: 99,
                channel_id: 11,
                parent_channel_id: None,
                newest_message_id: Some(22),
                oldest_message_id: Some(11),
                historical_complete: false,
                archived_message_count: Some(50),
                archived_byte_count: Some(8192),
            }],
        };

        let state = checkpoint_state(&checkpoint, &target).expect("target state should exist");
        assert!(super::target_has_resume_state(state));
    }

    #[test]
    // archive[verify sync.progress.estimated-telemetry]
    fn estimate_target_total_messages_uses_checkpoint_density() {
        let target = test_target(1_300_000_000_000_000_000, 99, false);
        let state = SyncTargetCheckpoint {
            guild_id: 99,
            channel_id: target.channel_id(),
            parent_channel_id: None,
            newest_message_id: Some(1_400_000_000_000_000_000),
            oldest_message_id: Some(1_350_000_000_000_000_000),
            historical_complete: false,
            archived_message_count: Some(200),
            archived_byte_count: Some(200 * 4096),
        };

        let estimate = estimate_target_total_messages(&target, Some(&state), 100);
        assert!(estimate > 200);
    }

    #[test]
    // archive[verify sync.progress.estimated-telemetry]
    fn overall_progress_metrics_reports_rates_and_remaining_estimates() {
        let targets = vec![
            test_target(1_300_000_000_000_000_000, 99, false),
            test_target(1_300_000_000_100_000_000, 99, false),
        ];
        let checkpoint = SyncCheckpoint {
            version: CHECKPOINT_VERSION,
            targets: vec![SyncTargetCheckpoint {
                guild_id: 99,
                channel_id: targets[0].channel_id(),
                parent_channel_id: None,
                newest_message_id: Some(1_400_000_000_000_000_000),
                oldest_message_id: Some(1_350_000_000_000_000_000),
                historical_complete: false,
                archived_message_count: Some(200),
                archived_byte_count: Some(200 * 4096),
            }],
        };
        let tracker = SyncProgressTracker {
            started_at: Instant::now() - std::time::Duration::from_secs(10),
            targets_total: 2,
            resumed_targets: 1,
            targets_completed: 0,
        };

        let metrics = overall_progress_metrics(&targets, &checkpoint, &tracker);
        assert_eq!(metrics.resumed_targets, 1);
        assert_eq!(metrics.messages_processed, 200);
        assert!(metrics.estimated_messages_total >= metrics.messages_processed);
        assert!(metrics.estimated_messages_remaining > 0);
        assert!(metrics.bytes_processed > 0);
        assert!(metrics.message_rate_per_sec > 0);
        assert!(metrics.eta_known);
    }

    #[test]
    fn missing_access_error_code_is_recognized() {
        assert!(is_missing_access_error_code(Some(50_001)));
        assert!(!is_missing_access_error_code(Some(50_013)));
        assert!(!is_missing_access_error_code(None));
    }
}
