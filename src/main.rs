use std::cmp::{max, min};
use std::fs;
use std::{error::Error, io, io::stdout};

use color_eyre::config::HookBuilder;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{prelude::*, widgets::*};

use rusqlite::Connection;

use clipboard::{ClipboardContext, ClipboardProvider};

static DICPATH: &str = "/usr/share/dicrs/";
static DICEXTENSION: &str = ".db";
// static APPTERMTITLE: &str = "\x1b]0;dic.rs\x07";

struct App {
    input: String,
    definition: String,
    selected_index: usize,
    dictionary_index: usize,
    database_path: String,
    conn: Connection,
    word_index: Vec<String>,
    databases: Vec<String>,
}

#[derive(Default)]
struct DicEntry {
    index: usize,
    word: String,
    definition: String,
}

fn list_databases() -> Vec<String> {
    let mut res: Vec<String> = Vec::new();
    for entry in fs::read_dir(DICPATH).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let filename = path.file_name().ok_or("No filename").unwrap().to_str();
        let filename = filename.unwrap().to_string().replace(DICEXTENSION, "");
        res.push(filename);
    }
    res
}

fn retrieve_db_index(conn: &Connection) -> Vec<String> {
    let mut stmt = conn.prepare("SELECT word FROM dictionary").unwrap();
    let mut rows = stmt.query([]).unwrap();
    let mut index = Vec::new();
    while let Ok(Some(row)) = rows.next() {
        index.push(row.get(0).unwrap());
    }
    index
}

fn main() -> Result<(), Box<dyn Error>> {
    let ctx: ClipboardContext = ClipboardProvider::new().unwrap();
    init_error_hooks()?;
    let terminal = init_terminal()?;
    let mut app = App::default();
    app.create([DICPATH, app.databases.first().unwrap(), DICEXTENSION].concat());
    app.run(terminal, ctx)?;

    restore_terminal()?;

    Ok(())
}

fn init_error_hooks() -> color_eyre::Result<()> {
    let (panic, error) = HookBuilder::default().into_hooks();
    let panic = panic.into_panic_hook();
    let error = error.into_eyre_hook();
    color_eyre::eyre::set_hook(Box::new(move |e| {
        let _ = restore_terminal();
        error(e)
    }))?;
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        panic(info);
    }));
    Ok(())
}

fn init_terminal() -> color_eyre::Result<Terminal<impl Backend>> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal() -> color_eyre::Result<()> {
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

impl App {
    fn default() -> Self {
        Self {
            input: String::new(),
            definition: String::new(),
            selected_index: usize::default(),
            dictionary_index: usize::default(),
            database_path: String::new(),
            conn: Connection::open_in_memory().unwrap(),
            word_index: Vec::new(),
            databases: list_databases(),
        }
    }

    fn create(&mut self, db_path: String) {
        self.selected_index = 0;
        self.database_path.clone_from(&db_path);
        self.conn = Connection::open(&db_path).unwrap();
        self.word_index = retrieve_db_index(&self.conn);
        self.update_by_index(0);
    }

    fn update_by_index(&mut self, i: i32) {
        let mut new_index: i32 = max(0, self.selected_index as i32 + i);
        new_index = min(new_index, self.word_index.len() as i32 - 1);
        self.selected_index = new_index as usize;
        self.definition = self.query_db_by_index(self.selected_index + 1).definition;
    }

    fn change_database(&mut self, i: i32) {
        let x = self.dictionary_index as i32 + i;
        self.dictionary_index = if x == -1 {
            self.databases.len() - 1
        } else if x > self.databases.len() as i32 - 1 {
            0
        } else {
            (x % self.databases.len() as i32) as usize
        };
        self.create(
            [
                DICPATH,
                self.databases.get(self.dictionary_index).unwrap(),
                DICEXTENSION,
            ]
            .concat(),
        );
    }

    fn query_db(&mut self, word: String) -> DicEntry {
        let sql = "SELECT ROWID, word, definition FROM dictionary WHERE word LIKE :query";
        let wild_card_query = format!("{}%", word);
        let mut stmt = self.conn.prepare(sql).unwrap();
        let mut res = DicEntry::default();
        let mut rows = stmt
            .query_map([(wild_card_query)], |row| {
                let rowid: u32 = row.get(0)?;
                let word: String = row.get(1)?;
                let def: String = row.get(2)?;
                Ok((rowid, word, def))
            })
            .unwrap();

        if let Some(row) = rows.next() {
            let (rowid, word, def) = row.unwrap();
            res.index = (rowid - 1) as usize;
            res.word = word;
            res.definition = def.replace('\r', "\n");
        } else {
            res.definition = "Not found!".to_string();
        }
        res
    }
    fn query_db_by_index(&mut self, word: usize) -> DicEntry {
        let sql = "SELECT ROWID, word, definition FROM dictionary WHERE ROWID = :query";
        let wild_card_query = word.to_string();
        let mut stmt = self.conn.prepare(sql).unwrap();
        let mut res = DicEntry::default();
        let mut rows = stmt
            .query_map([(wild_card_query)], |row| {
                let rowid: u32 = row.get(0)?;
                let word: String = row.get(1)?;
                let def: String = row.get(2)?;
                Ok((rowid, word, def))
            })
            .unwrap();

        if let Some(row) = rows.next() {
            let (rowid, word, def) = row.unwrap();
            res.index = (rowid - 1) as usize;
            res.word = word;
            res.definition = def.replace('\r', "\n");
        } else {
            res.definition = "Not found!".to_string();
        }
        res
    }

    fn run(
        &mut self,
        mut terminal: Terminal<impl Backend>,
        mut ctx: ClipboardContext,
    ) -> io::Result<()> {
        loop {
            self.draw(&mut terminal)?;
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    use KeyCode::*;
                    match (key.code, key.modifiers) {
                        (Char('c'), KeyModifiers::CONTROL) => return Ok(()),
                        (Char('y'), KeyModifiers::CONTROL) => {
                            ctx.set_contents(self.definition.to_owned()).unwrap()
                        }
                        (Up, KeyModifiers::NONE) => self.update_by_index(-1),
                        (Down, KeyModifiers::NONE) => self.update_by_index(1),
                        (Up, KeyModifiers::SHIFT) => self.update_by_index(-10),
                        (Down, KeyModifiers::SHIFT) => self.update_by_index(10),
                        (Left, KeyModifiers::NONE) => self.change_database(-1),
                        (Right, KeyModifiers::NONE) => self.change_database(1),
                        (Enter, KeyModifiers::NONE) => {
                            let query_term: String = self.input.drain(..).collect();
                            let entry = self.query_db(query_term);
                            self.definition = entry.definition;
                            self.selected_index = entry.index;
                        }
                        (Backspace, _) => {
                            self.input.pop();
                        }
                        (Char(c), _) => self.input.push(c),
                        _ => {}
                    }
                }
            }
        }
    }

    fn draw(&mut self, terminal: &mut Terminal<impl Backend>) -> io::Result<()> {
        terminal.draw(|f| ui(f, self))?;
        Ok(())
    }
}

fn ui(f: &mut Frame, app: &App) {
    let vertical = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length((min(4, app.databases.len()) + 2) as u16),
        Constraint::Min(1),
    ]);
    let [help_area, input_area, databases_area, rest_area] = vertical.areas(f.size());

    let vertical = Layout::horizontal([Constraint::Length(18), Constraint::Min(0)]);
    let [words_area, definition_area] = vertical.areas(rest_area);

    let (msg, style) = (
        vec![
            "Press ".into(),
            "Ctrl-C".bold(),
            " to leave, ".into(),
            "Ctrl-Y".bold(),
            " to copy definition, ".into(),
            "Left/Right".bold(),
            " to change dictionary. ".into(),
        ],
        Style::default(),
    );
    let text = Text::from(Line::from(msg)).patch_style(style);
    let help_message = Paragraph::new(text);
    f.render_widget(help_message, help_area);

    let input = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(Color::LightCyan))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Reset))
                .title("Word"),
        );
    f.render_widget(input, input_area);

    let databases = List::new(app.databases.clone())
        .block(Block::default().borders(Borders::ALL).title("Dictionaries"))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    let mut state = ListState::default().with_selected(Some(app.dictionary_index));
    f.render_stateful_widget(databases, databases_area, &mut state);

    let word_index: Vec<String> = (app.word_index[max(
        app.selected_index as i32 - words_area.as_size().height as i32,
        0,
    ) as usize
        ..min(
            app.selected_index + words_area.as_size().height as usize,
            app.word_index.len(),
        )])
        .to_vec();
    let words_index = List::new(word_index)
        .block(Block::default().borders(Borders::ALL).title("Index"))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::White));
    let mut state = ListState::default().with_selected(Some(min(
        app.selected_index,
        words_area.as_size().height as usize,
    )));
    f.render_stateful_widget(words_index, words_area, &mut state);

    let definition = Paragraph::new(app.definition.as_str())
        .block(Block::default().borders(Borders::ALL).title("Definition"))
        .wrap(Wrap { trim: true });
    f.render_widget(definition, definition_area);
}
