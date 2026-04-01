use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;

/// Show the effective output directory.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct OutputDirShowArgs;

impl OutputDirShowArgs {
    /// # Errors
    ///
    /// This function will return an error if no output directory has been configured.
    #[expect(clippy::unused_async)]
    pub async fn invoke(self) -> Result<()> {
        let resolved = crate::paths::require_output_dir(None)?;
        println!("{}", resolved.path.display());
        Ok(())
    }
}
