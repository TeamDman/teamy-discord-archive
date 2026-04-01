use arbitrary::Arbitrary;
use eyre::Context;
use eyre::Result;
use facet::Facet;

/// Open the effective output directory in the platform file manager.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct OutputDirOpenArgs;

impl OutputDirOpenArgs {
    /// # Errors
    ///
    /// This function will return an error if no output directory has been configured,
    /// if the directory cannot be created, or if the file manager cannot be launched.
    #[expect(clippy::unused_async)]
    pub async fn invoke(self) -> Result<()> {
        let resolved = crate::paths::require_output_dir(None)?;
        resolved.ensure_dir()?;
        open::that_detached(resolved.path.as_path()).wrap_err_with(|| {
            format!("Failed to open {} in file manager", resolved.path.display())
        })?;
        Ok(())
    }
}
