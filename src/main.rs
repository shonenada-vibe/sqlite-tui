mod app;
mod db;
mod editor;
mod ui;

use anyhow::{Result, bail};
use app::{App, Focus};
use clap::Parser;
use crossterm::{
    event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind},
    execute,
};
use db::Database;
use std::{
    io::{self, IsTerminal},
    path::PathBuf,
    time::Duration,
};

#[derive(Parser)]
#[command(
    version,
    about = "A keyboard-driven SQLite browser and SQL workbench",
    after_help = "Without a database path, opens an in-memory demo. Press ? inside the app for shortcuts."
)]
struct Args {
    /// SQLite database file to open
    #[arg(value_name = "DATABASE", required_if_eq("create", "true"))]
    database: Option<PathBuf>,
    /// Prevent writes to the database
    #[arg(long, short = 'r', conflicts_with = "create", requires = "database")]
    read_only: bool,
    /// Allow creating a new database file
    #[arg(long, requires = "database")]
    create: bool,
    /// Maximum SQL execution time in seconds
    #[arg(long, default_value = "10", value_parser = clap::value_parser!(u64).range(1..=3600))]
    timeout: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!(
            "sqlite-tui needs an interactive terminal. Run it directly in your terminal (try --help for usage)."
        );
    }
    let timeout = Duration::from_secs(args.timeout);
    let (db, label) = match args.database {
        Some(path) => (
            Database::open(&path, args.read_only, args.create, timeout)?,
            path.display().to_string(),
        ),
        None => (Database::demo(timeout)?, "demo · in memory".into()),
    };
    let mut app = App::new(db, label, args.read_only)?;
    let mut terminal = ratatui::init();
    let _guard = TerminalGuard;
    execute!(io::stdout(), EnableBracketedPaste)?;
    while !app.quit {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => app.key(key),
            Event::Paste(text)
                if app.focus == Focus::Editor && app.popup.is_none() && !app.searching =>
            {
                app.editor.insert(&text)
            }
            _ => {}
        }
    }
    Ok(())
}

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableBracketedPaste);
        ratatui::restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_cli_modes() {
        assert!(
            Args::try_parse_from(["sqlite-tui"])
                .unwrap()
                .database
                .is_none()
        );
        assert!(Args::try_parse_from(["sqlite-tui", "--create"]).is_err());
        assert!(Args::try_parse_from(["sqlite-tui", "--read-only"]).is_err());
        assert!(
            Args::try_parse_from(["sqlite-tui", "--create", "--read-only", "test.db"]).is_err()
        );
        assert!(Args::try_parse_from(["sqlite-tui", "--timeout", "0"]).is_err());
        assert!(Args::try_parse_from(["sqlite-tui", "--create", "test.db"]).is_ok());
    }
}
