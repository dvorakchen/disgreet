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
    let args = Args::parse();
    Disgreet::new(&args.background).run()?;

    Ok(())
}
