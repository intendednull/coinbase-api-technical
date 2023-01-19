//! Terminal user interface

use std::{io::Stdout, time::Duration};

use crossterm::{
    event::{self, poll, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use itertools::{EitherOrBoth, Itertools};
use std::io;
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
    Frame, Terminal,
};

use crate::model::OrderBook;

/// All state needed for TUI App.
pub struct App {
    state: TableState,
    pub order_book: OrderBook,
}

impl App {
    pub fn new() -> App {
        App {
            state: TableState::default(),
            order_book: OrderBook::default(),
        }
    }

    /// Select the next item in the table.
    pub fn next(&mut self) {
        let size = self.order_book.bids.len().max(self.order_book.asks.len());
        let i = match self.state.selected() {
            Some(i) => {
                if i >= size - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    /// Select the previous item in the table.
    pub fn previous(&mut self) {
        let size = self.order_book.bids.len().max(self.order_book.asks.len());
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    size - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }
}

/// Setup terminal for UI rendering.
pub fn setup() -> eyre::Result<Terminal<CrosstermBackend<Stdout>>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

/// Return the terminal to normal.
pub fn teardown<B>(mut terminal: Terminal<B>) -> eyre::Result<()>
where
    B: Backend + std::io::Write,
{
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Read input key code without blocking.
fn read_key_code() -> eyre::Result<Option<KeyCode>> {
    if poll(Duration::from_millis(100))? {
        // It's guaranteed that `read` won't block, because `poll` returned
        // `Ok(true)`.
        if let Event::Key(key) = event::read()? {
            Ok(Some(key.code))
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

/// Update a single frame of the UI, returning whether or not we should continue rendering.
pub fn update_frame<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> eyre::Result<bool> {
    terminal.draw(|f| ui(f, app))?;

    if let Some(code) = read_key_code()? {
        match code {
            KeyCode::Char('q') => return Ok(false),
            KeyCode::Down | KeyCode::Char('j') => app.next(),
            KeyCode::Up | KeyCode::Char('k') => app.previous(),
            _ => {}
        };
    }

    Ok(true)
}

/// Build UI layout
fn ui<B: Backend>(frame: &mut Frame<B>, app: &mut App) {
    let rects = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(80), Constraint::Percentage(20)].as_ref())
        .margin(2)
        .split(frame.size());
    let selected_style = Style::default().add_modifier(Modifier::REVERSED);
    let normal_style = Style::default().bg(Color::Blue);
    let header_cells = ["Bid Price", "Bid Size", "Ask Price", "Ask Size"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Red)));
    let header = Row::new(header_cells)
        .style(normal_style)
        .height(1)
        .bottom_margin(1);
    // Accumulate stats here. A little ugly, but allows us to iterate over the entire order book
    // only once.
    let mut total_bid_size = 0.;
    let mut total_ask_size = 0.;
    let rows = app
        .order_book
        .bids
        .iter()
        .rev()
        .zip_longest(app.order_book.asks.iter())
        .map(|item| match item {
            EitherOrBoth::Both(bid, ask) => {
                total_ask_size += ask.size;
                total_bid_size += bid.size;

                let cells = [
                    bid.price.to_string(),
                    bid.size.to_string(),
                    ask.price.to_string(),
                    ask.size.to_string(),
                ]
                .into_iter()
                .map(Cell::from);

                Row::new(cells).bottom_margin(1)
            }
            EitherOrBoth::Left(bid) => {
                total_bid_size += bid.size;

                let cells = [bid.price.to_string(), bid.size.to_string()]
                    .into_iter()
                    .map(Cell::from);

                Row::new(cells)
            }
            EitherOrBoth::Right(ask) => {
                total_ask_size += ask.size;

                let cells = [ask.price.to_string(), ask.size.to_string()]
                    .into_iter()
                    .map(Cell::from);

                Row::new(cells)
            }
        });

    let table = Table::new(rows)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(app.order_book.product_id.clone()),
        )
        .highlight_style(selected_style)
        .highlight_symbol(">> ")
        .widths(&[
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ]);

    frame.render_stateful_widget(table, rects[0], &mut app.state);

    let percent_spread = {
        let best_bid = app.order_book.bids.last().copied().unwrap_or_default();
        let best_ask = app.order_book.asks.first().copied().unwrap_or_default();

        // Calculate percent spread, while avoiding divide by zero errors.
        100. - (best_ask.price / best_bid.price) * 100.
    };
    let text = vec![
        Spans::from(Span::styled(
            format!("Total Bid Size: {total_bid_size}"),
            Style::default().bg(Color::Black).fg(Color::White),
        )),
        Spans::from(Span::styled(
            format!("Total Ask Size: {total_ask_size}"),
            Style::default().bg(Color::Black).fg(Color::White),
        )),
        Spans::from(Span::styled(
            format!("Percent Spread: {percent_spread}"),
            Style::default().bg(Color::Black).fg(Color::White),
        )),
    ];
    let create_block = |title| {
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black).fg(Color::White))
            .title(Span::styled(
                title,
                Style::default().add_modifier(Modifier::BOLD),
            ))
    };
    let paragraph = Paragraph::new(text.clone())
        .style(Style::default().bg(Color::White).fg(Color::Black))
        .block(create_block(""))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, rects[1]);
}
