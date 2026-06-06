use std::env;

use clap::Parser;
use disgreet::Disgreet;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// path of the config file
    #[arg(short, long, default_value = "")]
    background: String,
}

fn main() -> anyhow::Result<()> {
    let lang = env::var("LANG")
        .unwrap_or("en-us".to_string())
        .to_lowercase();
    let lang = lang.split('.').next().unwrap_or_default();

    let args = Args::parse();
    Disgreet::new(lang, &args.background).run()?;

    Ok(())
}
