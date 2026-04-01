use eyre::Context;

/// # Errors
///
/// This function will return an error if the value cannot be serialized to facet JSON.
pub fn print_facet_json<'facet, T>(value: &T) -> eyre::Result<()>
where
    T: facet::Facet<'facet> + ?Sized,
{
    let json =
        facet_json::to_string_pretty(value).wrap_err("Failed to serialize facet JSON output")?;
    println!("{json}");
    Ok(())
}

/// # Errors
///
/// This function will return an error if the value cannot be serialized to serde JSON.
pub fn print_serde_json<T>(value: &T) -> eyre::Result<()>
where
    T: serde::Serialize + ?Sized,
{
    let json = serde_json::to_string_pretty(value).wrap_err("Failed to serialize JSON output")?;
    println!("{json}");
    Ok(())
}
