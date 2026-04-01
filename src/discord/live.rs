use chrono::DateTime;
use chrono::Utc;
use eyre::Context;
use eyre::bail;
use facet::Facet;
use serenity::all::Attachment;
use serenity::all::ChannelId;
use serenity::all::GuildInfo;
use serenity::all::Http;
use serenity::all::MessageId;
use serenity::http::GuildPagination;

const GUILDS_PAGE_LIMIT: u64 = 200;
const GUILDS_PAGE_LIMIT_USIZE: usize = 200;
const GUILD_MEMBERS_PAGE_LIMIT: u64 = 1_000;
const DISCORD_EPOCH_MILLIS: i64 = 1_420_070_400_000;
const DEFAULT_MESSAGE_LIMIT: u8 = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveDiscordClientConfig {
    pub token: String,
}

impl LiveDiscordClientConfig {
    #[must_use]
    pub fn http(&self) -> Http {
        Http::new(&self.token)
    }
}

#[derive(Facet, Clone, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct LiveAttachmentRecord {
    pub channel_id: u64,
    pub message_id: u64,
    pub attachment_id: u64,
    pub filename: String,
    pub description: Option<String>,
    pub size: u32,
    pub url: String,
    pub proxy_url: String,
    pub content_type: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub ephemeral: bool,
    pub duration_secs: Option<f64>,
    pub waveform: Option<Vec<u8>>,
}

impl LiveAttachmentRecord {
    #[must_use]
    pub fn from_attachment(channel_id: u64, message_id: u64, attachment: Attachment) -> Self {
        Self {
            channel_id,
            message_id,
            attachment_id: attachment.id.get(),
            filename: attachment.filename,
            description: attachment.description,
            size: attachment.size,
            url: attachment.url,
            proxy_url: attachment.proxy_url,
            content_type: attachment.content_type,
            width: attachment.width,
            height: attachment.height,
            ephemeral: attachment.ephemeral,
            duration_secs: attachment.duration_secs,
            waveform: attachment.waveform,
        }
    }
}

/// # Errors
///
/// This function will return an error if the before value is not a valid RFC3339 timestamp
/// or if it predates the Discord epoch.
pub fn parse_before_datetime(before: Option<&str>) -> eyre::Result<Option<DateTime<Utc>>> {
    before
        .map(|value| {
            let parsed = DateTime::parse_from_rfc3339(value)
                .wrap_err_with(|| format!("Failed to parse --before value {value:?} as RFC3339"))?;
            Ok(parsed.with_timezone(&Utc))
        })
        .transpose()
}

/// # Errors
///
/// This function will return an error if the timestamp predates the Discord epoch.
pub fn before_datetime_to_message_id(before: DateTime<Utc>) -> eyre::Result<MessageId> {
    let timestamp_millis = before.timestamp_millis();
    if timestamp_millis < DISCORD_EPOCH_MILLIS {
        bail!("--before must not be earlier than the Discord epoch")
    }

    let snowflake = (timestamp_millis - DISCORD_EPOCH_MILLIS).cast_unsigned() << 22;
    Ok(MessageId::new(snowflake))
}

/// # Errors
///
/// This function will return an error unless exactly one of channel or thread id is provided.
pub fn resolve_message_target(
    channel_id: Option<u64>,
    thread_id: Option<u64>,
) -> eyre::Result<ChannelId> {
    match (channel_id, thread_id) {
        (Some(channel_id), None) => Ok(ChannelId::new(channel_id)),
        (None, Some(thread_id)) => Ok(ChannelId::new(thread_id)),
        (None, None) => bail!("Pass exactly one of --channel-id or --thread-id."),
        (Some(_), Some(_)) => bail!("Pass exactly one of --channel-id or --thread-id, not both."),
    }
}

#[must_use]
pub fn normalize_message_limit(limit: Option<u8>) -> u8 {
    limit.unwrap_or(DEFAULT_MESSAGE_LIMIT).clamp(1, 100)
}

/// # Errors
///
/// This function will return an error if the API call fails.
pub async fn list_guilds(http: &Http) -> eyre::Result<Vec<GuildInfo>> {
    let mut guilds = Vec::new();
    let mut cursor = None;

    loop {
        let page = http
            .get_guilds(cursor.map(GuildPagination::After), Some(GUILDS_PAGE_LIMIT))
            .await
            .wrap_err("Failed to list guilds visible to the bot")?;

        if page.is_empty() {
            break;
        }

        cursor = page.last().map(|guild| guild.id);
        let page_len = page.len();
        guilds.extend(page);
        if page_len < GUILDS_PAGE_LIMIT_USIZE {
            break;
        }
    }

    Ok(guilds)
}

#[must_use]
pub fn normalize_user_limit(limit: Option<u64>) -> u64 {
    limit
        .unwrap_or(GUILD_MEMBERS_PAGE_LIMIT)
        .clamp(1, GUILD_MEMBERS_PAGE_LIMIT)
}

#[cfg(test)]
mod tests {
    use super::DISCORD_EPOCH_MILLIS;
    use super::before_datetime_to_message_id;
    use super::normalize_message_limit;
    use super::normalize_user_limit;
    use super::parse_before_datetime;
    use super::resolve_message_target;
    use chrono::TimeZone;
    use chrono::Utc;

    #[test]
    fn resolve_message_target_requires_exactly_one_id() {
        assert!(resolve_message_target(None, None).is_err());
        assert!(resolve_message_target(Some(1), Some(2)).is_err());
        assert_eq!(
            resolve_message_target(Some(42), None)
                .expect("channel id should resolve")
                .get(),
            42
        );
    }

    #[test]
    fn before_datetime_to_message_id_is_monotonic() {
        let earlier = Utc
            .timestamp_millis_opt(DISCORD_EPOCH_MILLIS + 1_000)
            .single()
            .expect("earlier timestamp should exist");
        let later = Utc
            .timestamp_millis_opt(DISCORD_EPOCH_MILLIS + 2_000)
            .single()
            .expect("later timestamp should exist");

        assert!(
            before_datetime_to_message_id(earlier)
                .expect("earlier snowflake should exist")
                .get()
                < before_datetime_to_message_id(later)
                    .expect("later snowflake should exist")
                    .get()
        );
    }

    #[test]
    fn parse_before_datetime_accepts_rfc3339() {
        let parsed = parse_before_datetime(Some("2024-01-01T00:00:00Z"))
            .expect("timestamp should parse")
            .expect("timestamp should exist");
        assert_eq!(parsed.to_rfc3339(), "2024-01-01T00:00:00+00:00");
    }

    #[test]
    fn normalize_message_limit_caps_to_discord_limit() {
        assert_eq!(normalize_message_limit(None), 100);
        assert_eq!(normalize_message_limit(Some(0)), 1);
        assert_eq!(normalize_message_limit(Some(200)), 100);
    }

    #[test]
    fn normalize_user_limit_caps_to_discord_limit() {
        assert_eq!(normalize_user_limit(None), 1_000);
        assert_eq!(normalize_user_limit(Some(0)), 1);
        assert_eq!(normalize_user_limit(Some(2_000)), 1_000);
    }
}
