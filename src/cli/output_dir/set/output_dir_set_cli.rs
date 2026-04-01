use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue::{self as args};

/// Persist the default output directory.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct OutputDirSetArgs {
    /// The path to persist as the default output directory.
    #[facet(args::positional)]
    pub path: String,
}

impl OutputDirSetArgs {
    /// # Errors
    ///
    /// This function will return an error if the preference cannot be written.
    pub fn persist(self, app_home: &crate::paths::AppHome) -> Result<()> {
        let path = std::path::PathBuf::from(self.path);
        crate::paths::save_output_dir_preference(app_home, path.as_path())?;
        println!("{}", path.display());
        Ok(())
    }

    /// # Errors
    ///
    /// This function will return an error if the preference cannot be written.
    #[expect(clippy::unused_async)]
    pub async fn invoke(self) -> Result<()> {
        self.persist(&crate::paths::APP_HOME)
    }
}
