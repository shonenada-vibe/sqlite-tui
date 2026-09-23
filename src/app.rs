use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::{ListState, TableState};

use crate::{
    db::{Database, Object, QueryResult, quote_identifier},
    editor::Editor,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Objects,
    Data,
    Editor,
}

pub struct Popup {
    pub title: String,
    pub text: String,
    pub scroll: u16,
}

pub struct App {
    pub db: Database,
    pub label: String,
    pub read_only: bool,
    pub objects: Vec<Object>,
    pub filtered: Vec<usize>,
    pub list: ListState,
    pub filter: String,
    pub searching: bool,
    pub focus: Focus,
    pub editor: Editor,
    pub result: QueryResult,
    pub table: TableState,
    pub column: usize,
    pub first_column: usize,
    pub opened: Option<String>,
    pub page: usize,
    pub status: String,
    pub error: bool,
    pub popup: Option<Popup>,
    pub quit: bool,
    history: Vec<String>,
    history_index: usize,
    history_draft: String,
}

impl App {
    pub fn new(db: Database, label: String, read_only: bool) -> Result<Self> {
        let mut app = Self {
            db,
            label,
            read_only,
            objects: vec![],
            filtered: vec![],
            list: ListState::default(),
            filter: String::new(),
            searching: false,
            focus: Focus::Objects,
            editor: Editor::default(),
            result: QueryResult::default(),
            table: TableState::default(),
            column: 0,
            first_column: 0,
            opened: None,
            page: 0,
            status: "Ready · Select a table or press i to write SQL".into(),
            error: false,
            popup: None,
            quit: false,
            history: vec![],
            history_index: 0,
            history_draft: String::new(),
        };
        app.refresh_objects()?;
        if let Some(object) = app.selected_object() {
            app.editor
                .set(format!("SELECT * FROM {};", quote_identifier(&object.name)));
            app.open_selected()?;
        }
        Ok(app)
    }

    pub fn selected_object(&self) -> Option<&Object> {
        self.list
            .selected()
            .and_then(|i| self.filtered.get(i))
            .and_then(|i| self.objects.get(*i))
    }

    fn refresh_objects(&mut self) -> Result<()> {
        let selected = self.selected_object().map(|o| o.name.clone());
        self.objects = self.db.objects()?;
        self.apply_filter();
        if let Some(index) = self
            .filtered
            .iter()
            .position(|i| Some(&self.objects[*i].name) == selected.as_ref())
        {
            self.list.select(Some(index));
        }
        Ok(())
    }

    fn apply_filter(&mut self) {
        let filter = self.filter.to_lowercase();
        self.filtered = self
            .objects
            .iter()
            .enumerate()
            .filter(|(_, o)| o.name.to_lowercase().contains(&filter))
            .map(|(i, _)| i)
            .collect();
        self.list.select(if self.filtered.is_empty() {
            None
        } else {
            Some(0)
        });
    }

    fn open_selected(&mut self) -> Result<()> {
        if let Some(name) = self.selected_object().map(|o| o.name.clone()) {
            let result = self.db.browse(&name, 0)?;
            self.page = 0;
            self.opened = Some(name);
            self.set_result(result);
        }
        Ok(())
    }

    fn set_result(&mut self, result: QueryResult) {
        self.status = format!(
            "{} rows{} · {:.1} ms{}",
            result.rows.len(),
            if result.truncated { "+" } else { "" },
            result.elapsed.as_secs_f64() * 1_000.0,
            result
                .affected
                .map(|n| format!(" · {n} changes"))
                .unwrap_or_default()
        );
        self.error = false;
        self.table = TableState::default().with_selected(if result.rows.is_empty() {
            None
        } else {
            Some(0)
        });
        self.column = 0;
        self.first_column = 0;
        self.result = result;
    }

    fn run_query(&mut self) -> Result<()> {
        let sql = self.editor.text.clone();
        let result = self.db.query(&sql)?;
        if self.history.last() != Some(&sql) {
            self.history.push(sql);
        }
        self.history_index = self.history.len();
        self.history_draft.clear();
        self.opened = None;
        self.page = 0;
        self.set_result(result);
        self.refresh_objects()?;
        self.focus = Focus::Data;
        Ok(())
    }

    pub fn key(&mut self, key: KeyEvent) {
        if let Err(error) = self.handle_key(key) {
            self.status = format!("{error:#}");
            self.error = true;
            self.popup = Some(Popup {
                title: " SQL error ".into(),
                text: self.status.clone(),
                scroll: 0,
            });
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && matches!(key.code, KeyCode::Char('c' | 'q')) {
            self.quit = true;
            return Ok(());
        }
        if let Some(popup) = &mut self.popup {
            match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q' | '?') => self.popup = None,
                KeyCode::Down | KeyCode::Char('j') => popup.scroll = popup.scroll.saturating_add(1),
                KeyCode::Up | KeyCode::Char('k') => popup.scroll = popup.scroll.saturating_sub(1),
                KeyCode::PageDown => popup.scroll = popup.scroll.saturating_add(10),
                KeyCode::PageUp => popup.scroll = popup.scroll.saturating_sub(10),
                _ => {}
            }
            return Ok(());
        }
        if self.searching {
            match key.code {
                KeyCode::Esc => {
                    self.filter.clear();
                    self.searching = false;
                    self.apply_filter();
                }
                KeyCode::Enter => {
                    self.searching = false;
                    self.open_selected()?;
                }
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.apply_filter();
                }
                KeyCode::Char(c) if !ctrl => {
                    self.filter.push(c);
                    self.apply_filter();
                }
                KeyCode::Down => self.move_selection(1),
                KeyCode::Up => self.move_selection(-1),
                _ => {}
            }
            return Ok(());
        }
        if key.code == KeyCode::F(5) || (ctrl && key.code == KeyCode::Char('r')) {
            return self.run_query();
        }
        match key.code {
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Objects => Focus::Data,
                    Focus::Data => Focus::Editor,
                    Focus::Editor => Focus::Objects,
                };
                return Ok(());
            }
            KeyCode::BackTab => {
                self.focus = match self.focus {
                    Focus::Objects => Focus::Editor,
                    Focus::Data => Focus::Objects,
                    Focus::Editor => Focus::Data,
                };
                return Ok(());
            }
            KeyCode::Esc => {
                self.focus = Focus::Objects;
                return Ok(());
            }
            _ => {}
        }
        if self.focus == Focus::Editor {
            if ctrl && matches!(key.code, KeyCode::Char('p' | 'n')) {
                if self.history_index == self.history.len() {
                    self.history_draft = self.editor.text.clone();
                }
                self.history_index = if key.code == KeyCode::Char('p') {
                    self.history_index.saturating_sub(1)
                } else {
                    (self.history_index + 1).min(self.history.len())
                };
                self.editor.set(
                    self.history
                        .get(self.history_index)
                        .unwrap_or(&self.history_draft)
                        .clone(),
                );
            } else {
                self.editor.key(key);
            }
            return Ok(());
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') | KeyCode::F(1) => {
                self.popup = Some(Popup {
                    title: " Keyboard shortcuts ".into(),
                    text: HELP.into(),
                    scroll: 0,
                })
            }
            KeyCode::Char('i' | ':') => self.focus = Focus::Editor,
            KeyCode::Char('/') => {
                self.focus = Focus::Objects;
                self.searching = true;
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::PageDown => self.move_selection(10),
            KeyCode::PageUp => self.move_selection(-10),
            KeyCode::Home | KeyCode::Char('g') => self.move_selection(-isize::MAX),
            KeyCode::End | KeyCode::Char('G') => self.move_selection(isize::MAX),
            KeyCode::Left | KeyCode::Char('h') if self.focus == Focus::Data => {
                self.column = self.column.saturating_sub(1)
            }
            KeyCode::Right | KeyCode::Char('l') if self.focus == Focus::Data => {
                self.column = (self.column + 1).min(self.result.columns.len().saturating_sub(1))
            }
            KeyCode::Enter if self.focus == Focus::Objects => {
                self.open_selected()?;
                self.focus = Focus::Data;
            }
            KeyCode::Enter if self.focus == Focus::Data => {
                if let Some(value) = self
                    .table
                    .selected()
                    .and_then(|i| self.result.rows.get(i))
                    .and_then(|r| r.get(self.column))
                {
                    self.popup = Some(Popup {
                        title: format!(" {} ", self.result.columns[self.column]),
                        text: value.clone(),
                        scroll: 0,
                    });
                }
            }
            KeyCode::Char('s') => {
                if let Some(object) = self.selected_object() {
                    self.popup = Some(Popup {
                        title: format!(" Schema · {} ", object.name),
                        text: object.schema.clone(),
                        scroll: 0,
                    });
                }
            }
            KeyCode::Char('r') => {
                self.refresh_objects()?;
                if let Some(name) = &self.opened {
                    self.set_result(self.db.browse(name, self.page)?);
                } else {
                    self.status = "Table list refreshed · F5 runs the editor again".into();
                }
            }
            KeyCode::Char('[' | ']') => {
                if let Some(name) = &self.opened {
                    let next = key.code == KeyCode::Char(']');
                    if !next || self.result.truncated {
                        let page = if next {
                            self.page + 1
                        } else {
                            self.page.saturating_sub(1)
                        };
                        let result = self.db.browse(name, page)?;
                        self.page = page;
                        self.set_result(result);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn move_selection(&mut self, delta: isize) {
        let (selected, len) = if self.focus == Focus::Objects {
            (self.list.selected(), self.filtered.len())
        } else {
            (self.table.selected(), self.result.rows.len())
        };
        let next = if len == 0 {
            None
        } else {
            Some(
                selected
                    .unwrap_or(0)
                    .saturating_add_signed(delta)
                    .min(len - 1),
            )
        };
        if self.focus == Focus::Objects {
            self.list.select(next);
        } else {
            self.table.select(next);
        }
    }
}

const HELP: &str = "NAVIGATE\nTab / Shift+Tab    Switch between tables, results, and editor\nj k / ↑ ↓          Move through tables or rows\nh l / ← →          Select a result column\nEnter              Open a table / inspect a full cell\n[ / ]              Previous / next 100-row table page\nPgUp / PgDn        Move 10 rows\ng / G              First / last row\n/                  Filter tables and views\ns                  Show the selected object's CREATE statement\nr                  Refresh tables and the current table page\n\nSQL EDITOR\ni / :              Focus the editor\nF5 / Ctrl+R        Execute one SQL statement\nEnter              Insert a new line\nCtrl+A / Ctrl+E    Start / end of line\nCtrl+U             Clear the editor\nCtrl+P / Ctrl+N    Previous / next successful query\nEsc                Return to the table list\n\nGENERAL\n? / F1             Show this help (outside the editor)\nq                  Quit (outside the editor)\nCtrl+C / Ctrl+Q    Quit from anywhere\n\nQueries show at most 1,000 rows. Table browsing uses pages of 100.\nWrites take effect immediately unless you explicitly BEGIN a transaction.\nAn open transaction is rolled back when you quit.\nUse --read-only to open a database without allowing writes.\nUse --timeout SECONDS to change the default 10-second query timeout.";

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn app() -> App {
        App::new(
            Database::demo(Duration::from_secs(2)).unwrap(),
            "demo".into(),
            false,
        )
        .unwrap()
    }
    #[test]
    fn typing_does_not_trigger_navigation_commands() {
        let mut app = app();
        app.key(KeyCode::Char('i').into());
        app.editor.set(String::new());
        for c in "select 'q?'".chars() {
            app.key(KeyCode::Char(c).into());
        }
        assert_eq!(app.editor.text, "select 'q?'");
        assert!(!app.quit);
        assert!(app.popup.is_none());
    }
    #[test]
    fn filters_and_opens_tables() {
        let mut app = app();
        app.key(KeyCode::Char('/').into());
        for c in "orders".chars() {
            app.key(KeyCode::Char(c).into());
        }
        app.key(KeyCode::Enter.into());
        assert_eq!(app.opened.as_deref(), Some("orders"));
        app.key(KeyCode::Char(']').into());
        assert_eq!(app.page, 1);
        assert_eq!(app.result.rows[0][0], "101");
    }
    #[test]
    fn failed_query_preserves_result_and_can_be_dismissed() {
        let mut app = app();
        let previous = app.result.rows.clone();
        app.editor.set("INVALID SQL".into());
        app.key(KeyCode::F(5).into());
        assert!(app.error);
        assert_eq!(app.result.rows, previous);
        assert!(app.popup.is_some());
        app.key(KeyCode::Esc.into());
        app.editor.set("SELECT 123".into());
        app.key(KeyCode::F(5).into());
        assert_eq!(app.result.rows[0][0], "123");
        assert!(!app.error);
    }
}
