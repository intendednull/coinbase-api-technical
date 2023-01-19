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
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame, Terminal,
};

use crate::model::Snapshot;

pub struct App {
    state: TableState,
    pub order_book: Snapshot,
}

impl App {
    pub fn new() -> App {
        App {
            state: TableState::default(),
            order_book: Snapshot::default(),
        }
    }

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

pub fn setup() -> eyre::Result<Terminal<CrosstermBackend<Stdout>>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

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

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let rects = Layout::default()
        .constraints([Constraint::Percentage(100)].as_ref())
        .margin(5)
        .split(f.size());

    let selected_style = Style::default().add_modifier(Modifier::REVERSED);
    let normal_style = Style::default().bg(Color::Blue);
    let header_cells = ["Bid Price", "Bid Size", "Ask Price", "Ask Size"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Red)));
    let header = Row::new(header_cells)
        .style(normal_style)
        .height(1)
        .bottom_margin(1);
    let rows = app
        .order_book
        .bids
        .iter()
        .rev()
        .zip_longest(app.order_book.asks.iter())
        .map(|item| match item {
            EitherOrBoth::Both(bid, ask) => {
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
                let cells = [bid.price.to_string(), bid.size.to_string()]
                    .into_iter()
                    .map(Cell::from);

                Row::new(cells)
            }
            EitherOrBoth::Right(ask) => {
                let cells = [ask.price.to_string(), ask.size.to_string()]
                    .into_iter()
                    .map(Cell::from);

                Row::new(cells)
            }
        });

    let t = Table::new(rows)
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
    f.render_stateful_widget(t, rects[0], &mut app.state);
}
