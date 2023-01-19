mod cli;
mod client;
mod model;
mod tui;

use clap::Parser;
use client::Client;
use model::update_order_book;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Get args and setup client.
    let args = cli::Args::parse();
    let mut client = Client::subscribe(&args.identifier).await?;
    let mut app = tui::App::new();
    let mut terminal = tui::setup()?;

    // Run the UI
    loop {
        update_order_book(&mut app.order_book, &mut client).await?;
        let should_continue = tui::update_frame(&mut terminal, &mut app)?;
        if !should_continue {
            break;
        }
    }

    // Clean up ui and client
    tui::teardown(terminal)?;
    client.close().await?;

    Ok(())
}
