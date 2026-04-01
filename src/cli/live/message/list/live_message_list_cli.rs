use arbitrary::Arbitrary;
use eyre::Context;
use eyre::Result;
use facet::Facet;
use figue::{self as args};
use serenity::all::GetMessages;
use serenity::all::Http;

/// List messages in a channel or thread.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct LiveMessageListArgs {
    /// Channel id whose messages should be listed.
    #[facet(args::named)]
    pub channel_id: Option<u64>,

    /// Thread id whose messages should be listed.
    #[facet(args::named)]
    pub thread_id: Option<u64>,

    /// Return messages before this RFC3339 timestamp.
    #[facet(args::named)]
    pub before: Option<String>,

    /// Maximum number of messages to list.
    #[facet(args::named)]
    pub limit: Option<u8>,
}

impl LiveMessageListArgs {
    /// # Errors
    ///
    /// This function will return an error if argument resolution fails or the Discord API call fails.
    pub async fn invoke(self, http: &Http) -> Result<()> {
        let target = crate::discord::live::resolve_message_target(self.channel_id, self.thread_id)?;
        let before = crate::discord::live::parse_before_datetime(self.before.as_deref())?;
        let mut builder =
            GetMessages::new().limit(crate::discord::live::normalize_message_limit(self.limit));
        if let Some(before) = before {
            builder = builder.before(crate::discord::live::before_datetime_to_message_id(before)?);
        }
        let messages = target
            .messages(http, builder)
            .await
            .wrap_err_with(|| format!("Failed to list messages for channel {}", target.get()))?;
        crate::json_stdout::print_serde_json(&messages)
    }
}
