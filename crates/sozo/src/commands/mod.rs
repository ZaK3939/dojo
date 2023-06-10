use anyhow::Result;
use scarb::core::Config;

use crate::args::Commands;

pub(crate) mod build;
pub(crate) mod init;
pub(crate) mod migrate;
pub(crate) mod options;
pub(crate) mod test;

pub fn run(command: Commands, config: &Config) -> Result<()> {
    match command {
        Commands::Init(args) => args.run(),
        Commands::Test(args) => args.run(config),
        Commands::Build(args) => args.run(config),
        Commands::Migrate(args) => args.run(config),
    }
}