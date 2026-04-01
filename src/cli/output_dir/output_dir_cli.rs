use crate::cli::output_dir::open::OutputDirOpenArgs;
use crate::cli::output_dir::set::OutputDirSetArgs;
use crate::cli::output_dir::show::OutputDirShowArgs;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue as args;

/// Output directory preference commands.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct OutputDirArgs {
    /// The output-dir subcommand to run.
    #[facet(args::subcommand)]
    pub command: OutputDirCommand,
}

/// Output directory subcommands.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum OutputDirCommand {
    /// Open the effective output directory in the platform file manager.
    Open(OutputDirOpenArgs),
    /// Persist the default output directory.
    Set(OutputDirSetArgs),
    /// Show the effective output directory.
    Show(OutputDirShowArgs),
}

impl OutputDirArgs {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self) -> Result<()> {
        match self.command {
            OutputDirCommand::Open(args) => args.invoke().await?,
            OutputDirCommand::Set(args) => args.invoke().await?,
            OutputDirCommand::Show(args) => args.invoke().await?,
        }

        Ok(())
    }
}
