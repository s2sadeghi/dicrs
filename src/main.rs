use std::cmp::{max, min};
use std::path::PathBuf;
use std::{error::Error, io, io::stdout};
use std::{fs, path};

use color_eyre::config::HookBuilder;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, SetTitle,
    },
    ExecutableCommand,
};
use ratatui::{prelude::*, widgets::*};

use rusqlite::Connection;

#[cfg(feature = "clipboard")]
use clipboard::{ClipboardContext, ClipboardProvider};

static DICEXTENSION: &str = ".db";

#[cfg(feature = "leitner")]
mod leitner;
#[cfg(feature = "leitner")]
use leitner::Leitner;

#[derive(PartialEq)]
enum Mode {
    Default,
    Mono,
    #[cfg(feature = "leitner")]
    Leitner,
}
struct App {
    input: String,
    definition: String,
    selected_index: usize,
    dictionary_index: usize,
    dicpath: PathBuf,
    database_path: PathBuf,
    conn: Connection,
    word_index: Vec<String>,
    databases: Vec<String>,
    #[cfg(feature = "leitner")]
    leitner: Leitner,
    mode: Mode,
    scroll: u16,
    #[cfg(feature = "clipboard")]
    clipboard: Option<ClipboardContext>,
}

#[derive(Default)]
struct DicEntry {
    index: usize,
    word: String,
    definition: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let home_dir = std::env::var("HOME").expect("HOME environment variable not set");
    let dicpath: PathBuf = path::Path::new(&home_dir).join(".local/share/dicrs/dictionaries/");
    if !dicpath.exists() {
        fs::create_dir_all(&dicpath)?;
    }
    init_error_hooks()?;
    let terminal = init_terminal()?;
    crossterm::execute!(io::stdout(), SetTitle("dic.rs")).unwrap();
    let mut app = App::default(dicpath.clone());
    if app.databases.is_empty() {
        restore_terminal()?;
        return Err(Box::<dyn Error>::from(
            "No databases found in '.local/share/dicrs/dictionaries/'.",
        ));
    }
    app.create(dicpath.join([app.databases.first().unwrap(), DICEXTENSION].concat()));
    #[cfg(feature = "clipboard")]
    if let Ok(cb) = <ClipboardContext as ClipboardProvider>::new() {
        app.clipboard = Some(cb);
    }
    app.run(terminal)?;

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
    fn default(dicpath: PathBuf) -> Self {
        #[cfg(feature = "leitner")]
        let home_dir = std::env::var("HOME").expect("HOME environment variable not set");
        let mut databases: Vec<String> = Vec::new();
        for entry in fs::read_dir(&dicpath).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let filename = path.file_name().ok_or("No filename").unwrap().to_str();
            let filename = filename.unwrap().to_string().replace(DICEXTENSION, "");
            databases.push(filename);
        }
        let mode = if databases.len() > 1 {
            Mode::Default
        } else {
            Mode::Mono
        };
        Self {
            input: String::new(),
            definition: String::new(),
            selected_index: usize::default(),
            dictionary_index: usize::default(),
            dicpath,
            database_path: PathBuf::new(),
            conn: Connection::open_in_memory().unwrap(),
            word_index: Vec::new(),
            databases,
            #[cfg(feature = "leitner")]
            leitner: Leitner::new(
                path::Path::new(&home_dir).join(".local/share/dicrs/leitner.sqlite"),
            )
            .unwrap(),
            mode,
            scroll: 0,
            #[cfg(feature = "clipboard")]
            clipboard: None,
        }
    }

    fn create(&mut self, db_path: PathBuf) {
        self.selected_index = 0;
        self.database_path.clone_from(&db_path);
        self.conn = Connection::open(&db_path).unwrap();
        self.word_index = self.retrieve_db_index();
        self.update_by_index(0);
    }

    fn retrieve_db_index(&self) -> Vec<String> {
        let mut stmt = self.conn.prepare("SELECT word FROM dictionary").unwrap();
        let mut rows = stmt.query([]).unwrap();
        let mut index = Vec::new();
        while let Ok(Some(row)) = rows.next() {
            index.push(row.get(0).unwrap());
        }
        index
    }

    fn update_by_index(&mut self, i: isize) {
        self.selected_index = (self.selected_index as isize + i)
            .clamp(0, self.word_index.len() as isize - 1) as usize;
        self.definition = self.query_db_by_index(self.selected_index + 1).definition;
    }

    fn change_database(&mut self, i: isize) {
        let x = self.dictionary_index as isize + i;
        self.dictionary_index = if x == -1 {
            self.databases.len() - 1
        } else if x > self.databases.len() as isize - 1 {
            0
        } else {
            (x % self.databases.len() as isize) as usize
        };
        self.create(
            self.dicpath.join(
                [
                    self.databases.get(self.dictionary_index).unwrap(),
                    DICEXTENSION,
                ]
                .concat(),
            ),
        );
    }

    fn query_db(&mut self, word: String) {
        let sql = "SELECT ROWID, definition FROM dictionary WHERE word LIKE :query";
        let wild_card_query = format!("{}%", word);
        let mut stmt = self.conn.prepare(sql).unwrap();
        let mut rows = stmt
            .query_map([(wild_card_query)], |row| {
                let rowid: u32 = row.get(0)?;
                let def: String = row.get(1)?;
                Ok((rowid, def))
            })
            .unwrap();

        if let Some(row) = rows.next() {
            let (rowid, def) = row.unwrap();
            self.selected_index = (rowid - 1) as usize;
            self.definition = def.replace('\r', "\n");
        } else {
            self.definition = "Not found!".to_string();
        }
        self.scroll = 0;
    }
    fn query_db_by_index(&mut self, word_index: usize) -> DicEntry {
        let sql = "SELECT ROWID, word, definition FROM dictionary WHERE ROWID = :query";
        let wild_card_query = word_index.to_string();
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
        self.scroll = 0;
        res
    }

    fn run(&mut self, mut terminal: Terminal<impl Backend>) -> io::Result<()> {
        loop {
            self.draw(&mut terminal)?;
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    use KeyCode::*;
                    #[cfg(feature = "leitner")]
                    if self.mode == Mode::Leitner {
                        match (key.code, key.modifiers) {
                            (Char('c'), KeyModifiers::CONTROL) => return Ok(()),
                            (Char('y'), KeyModifiers::NONE) => {
                                let result = self.leitner.review(true);
                                if result.is_ok() {
                                    self.leitner.next()
                                }
                            }
                            (Char('n'), KeyModifiers::NONE) => {
                                let result = self.leitner.review(false);
                                if result.is_ok() {
                                    self.leitner.next()
                                }
                            }
                            (Char('l'), KeyModifiers::ALT) => {
                                self.mode = Mode::Default;
                                self.update_by_index(0);
                            }
                            (Char('m'), KeyModifiers::ALT) => {
                                self.mode = Mode::Mono;
                                self.update_by_index(0);
                            }
                            (Up, KeyModifiers::NONE) => self.leitner.update_index_by(-1),
                            (Down, KeyModifiers::NONE) => self.leitner.update_index_by(1),
                            (PageDown, KeyModifiers::NONE) => {
                                self.scroll += 1;
                            }
                            (PageUp, KeyModifiers::NONE) => {
                                self.scroll = self.scroll.saturating_sub(1)
                            }
                            (Enter, KeyModifiers::NONE) | (Char(' '), KeyModifiers::NONE) => {
                                self.definition =
                                    self.leitner.get_definition(self.leitner.selected_index);
                            }
                            _ => {}
                        }
                        continue;
                    }
                    match (key.code, key.modifiers) {
                        (Char('c'), KeyModifiers::CONTROL) => return Ok(()),
                        #[cfg(feature = "clipboard")]
                        (Char('y'), KeyModifiers::CONTROL) => {
                            if let Some(ref mut cb) = self.clipboard {
                                cb.set_contents(self.definition.to_owned()).unwrap()
                            }
                        }
                        (Char('m'), KeyModifiers::ALT) => {
                            self.mode = if self.mode != Mode::Mono {
                                Mode::Mono
                            } else {
                                Mode::Default
                            };
                        }
                        #[cfg(feature = "leitner")]
                        (Char('l'), KeyModifiers::ALT) => {
                            self.mode = Mode::Leitner;
                            self.leitner.next();
                            self.definition =
                                "Enter or Space: Show the definition of the selected word.\n\
Y: Mark the current word as \"correct\" and review it again later.\n\
N: Mark the current word as \"incorrect\" and review it sooner.\n\
Alt + L / Alt + M: Switch to the Default / Mono Mode.\n\
↑: Move the selection up in the word index.\n\
↓: Move the selection down in the word index.\n"
                                    .to_string();
                        }
                        #[cfg(feature = "leitner")]
                        (Char('`'), KeyModifiers::NONE) => {
                            let entry = self.query_db_by_index(self.selected_index + 1);
                            let _ = self.leitner.add(&entry.word, &entry.definition);
                        }
                        (Up, KeyModifiers::NONE) => self.update_by_index(-1),
                        (Down, KeyModifiers::NONE) => self.update_by_index(1),
                        (Up, KeyModifiers::SHIFT) => self.update_by_index(-10),
                        (Down, KeyModifiers::SHIFT) => self.update_by_index(10),
                        (Left, KeyModifiers::NONE) => {
                            self.change_database(-1);
                            self.query_db(self.input.to_string());
                        }
                        (Right, KeyModifiers::NONE) => {
                            self.change_database(1);
                            self.query_db(self.input.to_string());
                        }
                        (PageDown, KeyModifiers::NONE) => {
                            self.scroll += 1;
                        }
                        (PageUp, KeyModifiers::NONE) => self.scroll = self.scroll.saturating_sub(1),
                        (Enter, KeyModifiers::NONE) => {
                            self.query_db(self.input.to_string());
                        }
                        (Backspace, KeyModifiers::NONE) => {
                            self.input.pop();
                        }
                        (Backspace, KeyModifiers::ALT) => delete_last_word(&mut self.input),
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

fn ui(f: &mut Frame, app: &mut App) {
    match app.mode {
        Mode::Default => render_default_mode(f, app),
        Mode::Mono => render_mono_mode(f, app),
        #[cfg(feature = "leitner")]
        Mode::Leitner => render_leitner_mode(f, app),
    }
}

fn render_default_mode(f: &mut Frame, app: &mut App) {
    let vertical = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(5),
        Constraint::Length(1),
    ]);
    let [input_area, rest_area, databases_area] = vertical.areas(f.area());

    let vertical = Layout::horizontal([Constraint::Length(18), Constraint::Min(0)]);
    let [words_area, definition_area] = vertical.areas(rest_area);

    let input = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(Color::LightCyan))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Reset))
                .title("Input"),
        );
    f.render_widget(input, input_area);

    let highlighted_databases: Vec<Span> = app.databases
        .iter()
        .enumerate()
        .map(|(i, db)| {
            let db = db.to_string() + " ";
            if i == app.dictionary_index {
                Span::styled(db, Style::default().fg(Color::Yellow).bold())
            } else {
                Span::raw(db)
            }
        })
        .collect();
    let db_lengths: Vec<usize> = app.databases.iter().map(|db| db.len() + 1).collect();
    let total_length: usize = db_lengths.iter().sum();
    let selected_position: usize = db_lengths.iter().take(app.dictionary_index).sum();
    let viewport_width = databases_area.width as usize;
    
    let scroll_x = if viewport_width >= total_length {
        0 
    } else {
        selected_position.saturating_sub(viewport_width.saturating_sub(db_lengths[app.dictionary_index]) / 2)
    };
    let databases = Paragraph::new(Line::from(highlighted_databases)).scroll((0,scroll_x as u16));
    f.render_widget(databases, databases_area);

    let height = words_area.as_size().height as usize - 2;
    let before = max(app.selected_index as isize - height as isize / 2, 0) as usize;
    let after = min(app.selected_index + height, app.word_index.len());
    let word_index: Vec<String> = (app.word_index[before..after]).to_vec();
    let word_index = List::new(word_index)
        .block(Block::default().borders(Borders::ALL).title("Index"))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::White));
    let mut state = ListState::default().with_selected(Some(min(app.selected_index, height / 2)));
    f.render_stateful_widget(word_index, words_area, &mut state);
    let max_scroll = calculate_max_scroll(
        app.definition.as_str(),
        definition_area.width,
        definition_area.height,
    );
    app.scroll = app.scroll.min(max_scroll);
    let definition = Paragraph::new(app.definition.as_str())
        .block(Block::default().borders(Borders::ALL).title("Definition"))
        .scroll((app.scroll, 0))
        .wrap(Wrap { trim: true });
    f.render_widget(definition, definition_area);
}

fn render_mono_mode(f: &mut Frame, app: &mut App) {
    let vertical = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(5),
    ]);
    let [input_area, definition_area] = vertical.areas(f.area());

    let input = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(Color::LightCyan))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Reset))
                .title("Input"),
        );
    f.render_widget(input, input_area);
    let max_scroll = calculate_max_scroll(
        app.definition.as_str(),
        definition_area.width,
        definition_area.height,
    );
    app.scroll = app.scroll.min(max_scroll);

    let definition = Paragraph::new(app.definition.as_str())
        .block(Block::default().borders(Borders::ALL).title("Definition"))
        .scroll((app.scroll, 0))
        .wrap(Wrap { trim: true });
    f.render_widget(definition, definition_area);
}

#[cfg(feature = "leitner")]
fn render_leitner_mode(f: &mut Frame, app: &mut App) {
    let vertical = Layout::horizontal([Constraint::Length(18), Constraint::Min(24)]);
    let [words_area, definition_area] = vertical.areas(f.area());
    if app.leitner.word_index.is_empty() {
        let empty_list = List::new(vec![Span::from("Empty")])
            .block(Block::default().borders(Borders::ALL).title("Index"));
        f.render_widget(empty_list, words_area);

        let empty_definition = Paragraph::new("Use ~ (`) key to add a word to Leitner.")
            .block(Block::default().borders(Borders::ALL).title("Definition"))
            .wrap(Wrap { trim: true });
        f.render_widget(empty_definition, definition_area);

        return;
    }
    let height = words_area.as_size().height as usize - 2;
    let before = max(app.leitner.selected_index as isize - height as isize / 2, 0) as usize;
    let after = min(
        app.leitner.selected_index + height,
        app.leitner.word_index.len(),
    );
    let word_index: Vec<String> = app.leitner.word_index[before..after].to_vec();
    let word_index = List::new(word_index)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(leitner::get_box_symbol(
                    app.leitner.box_level[app.leitner.selected_index],
                ))
                .title_bottom(leitner::get_relative_date(
                    app.leitner.review_due[app.leitner.selected_index],
                )),
        )
        .highlight_style(Style::default().fg(Color::Black).bg(Color::White));
    let mut state =
        ListState::default().with_selected(Some(min(app.leitner.selected_index, height / 2)));
    f.render_stateful_widget(word_index, words_area, &mut state);
    let max_scroll = calculate_max_scroll(
        app.definition.as_str(),
        definition_area.width,
        definition_area.height,
    );
    app.scroll = app.scroll.min(max_scroll);

    let definition = Paragraph::new(app.definition.as_str())
        .block(Block::default().borders(Borders::ALL).title("Definition"))
        .scroll((app.scroll, 0))
        .wrap(Wrap { trim: true });
    f.render_widget(definition, definition_area);
}
fn calculate_max_scroll(content: &str, area_width: u16, area_height: u16) -> u16 {
    let wrapped_lines = content
        .lines()
        .map(|line| (line.len() as u16).div_ceil(area_width - 2))
        .sum::<u16>();
    (wrapped_lines).saturating_sub(area_height / 2)
}

fn delete_last_word(buffer: &mut String) {
    if let Some(pos) = buffer.rfind(|c: char| !c.is_whitespace()) {
        let last_space = buffer[..pos].rfind(|c: char| c.is_whitespace());
        match last_space {
            Some(idx) => buffer.truncate(idx + 1),
            None => buffer.clear(),
        }
    }
}
