pub mod container;
pub mod image;
pub mod pod;
pub mod runtime;
pub mod system;
pub mod volume;

use clap::ValueEnum;
use serde::Serialize;

#[derive(Clone, Debug, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
    Yaml,
}

pub fn print_structured<T: Serialize + ?Sized>(
    value: &T,
    format: &OutputFormat,
) -> anyhow::Result<()> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(value)?);
            Ok(())
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(value)?);
            Ok(())
        }
        OutputFormat::Table => anyhow::bail!("table output is not supported for this command"),
    }
}
