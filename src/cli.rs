use clap::Parser;

/// Live Order Book View
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// The crypto identifier to use (i.e. "ETH-USD")
    #[arg(short, long)]
    pub identifier: String,
}
