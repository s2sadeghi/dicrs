use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection, Result};
use std::path::PathBuf;
pub struct Leitner {
    conn: Connection,
    pub selected_index: usize,
    pub word_index: Vec<String>,
    pub review_due: Vec<NaiveDate>,
    pub box_level: Vec<u8>,
}

static INTERVALS: [u8; 5] = [1, 2, 4, 6, 10];

impl Leitner {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        let conn = Connection::open(&db_path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS cards (
                id INTEGER PRIMARY KEY,
                word TEXT NOT NULL,
                definition TEXT NOT NULL,
                box INTEGER NOT NULL DEFAULT 1,
                next_review DATE NOT NULL DEFAULT CURRENT_DATE,
                attempts INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;

        let mut stmt = conn.prepare("SELECT word, next_review, box FROM cards")?;
        let mut rows = stmt.query([])?;
        let mut word_index = Vec::new();
        let mut review_due = Vec::new();
        let mut box_level = Vec::new();
        let parse_from_str = NaiveDate::parse_from_str;
        while let Ok(Some(row)) = rows.next() {
            let word: String = row.get(0)?;
            let review_date_str: String = row.get(1)?;
            let box_n: u8 = row.get(2)?;
            let review_date: NaiveDate = parse_from_str(&review_date_str, "%Y-%m-%d").unwrap();
            word_index.push(word);
            review_due.push(review_date);
            box_level.push(box_n);
        }

        Ok(Self {
            conn: Connection::open(db_path)?,
            selected_index: 0,
            word_index,
            review_due,
            box_level,
        })
    }

    pub fn next(&mut self) {
        if self.word_index.is_empty() {
            return;
        }
        let today = chrono::Local::now().date_naive();
        while self.selected_index < self.word_index.len() - 1 {
            let review_date = &self.review_due[self.selected_index];
            if *review_date <= today {
                break;
            }
            self.selected_index += 1;
        }
    }

    pub fn update_index_by(&mut self, i: i32) {
        let new_index = (self.selected_index as i32 + i).clamp(0, self.word_index.len() as i32 - 1);
        self.selected_index = new_index as usize;
    }

    pub fn add(&mut self, word: &str, definition: &str) -> Result<()> {
        let review_date = chrono::Local::now().date_naive() + chrono::Duration::days(1.into());
        self.conn.execute(
            "INSERT INTO cards (word, definition, box, next_review) 
             VALUES (?1, ?2, 1, ?3)",
            params![word, definition, review_date.format("%Y-%m-%d").to_string()],
        )?;
        self.word_index.push(word.to_string());
        self.review_due.push(review_date);
        self.box_level.push(1);
        Ok(())
    }

    pub fn get_definition(&mut self, i: usize) -> String {
        let sql = "SELECT definition FROM cards WHERE ROWID = :query";
        let wild_card_query = (i + 1).to_string();
        let mut stmt = self.conn.prepare(sql).unwrap();
        let mut rows = stmt
            .query_map([(wild_card_query)], |row| {
                let def: String = row.get(0)?;
                Ok(def)
            })
            .unwrap();

        if let Some(row) = rows.next() {
            let def = row.unwrap();
            def.replace('\r', "\n")
        } else {
            "Not found!".to_string()
        }
    }
    pub fn review(&mut self, success: bool) -> Result<()> {
        if self.selected_index < self.word_index.len() {
            let today = chrono::Local::now().date_naive();
            let (review_date, current_box, attempts): (String, u8, u8) = self.conn.query_row(
                "SELECT next_review, box, attempts FROM cards WHERE ROWID = ?1",
                params![self.selected_index + 1],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            let mut review_date =
                chrono::NaiveDate::parse_from_str(&review_date, "%Y-%m-%d").unwrap();
            if review_date > today {
                return Ok(());
            }
            let (new_box, new_attempts) = if success {
                (current_box + 1, 0)
            } else if attempts + 1 >= 2 && current_box > 1 {
                (current_box - 1, 0)
            } else {
                (current_box, attempts + 1)
            };

            if new_box == 6 {
                self.conn.execute(
                    "DELETE FROM cards WHERE ROWID = ?1",
                    params![self.selected_index + 1],
                )?;
                self.word_index.remove(self.selected_index);
                self.review_due.remove(self.selected_index);
                self.box_level.remove(self.selected_index);
                if self.selected_index >= self.word_index.len() {
                    self.selected_index = self.word_index.len() - 1;
                }
            } else {
                let new_days = INTERVALS[(new_box - 1) as usize];
                review_date = today + chrono::Duration::days(new_days.into());
                self.conn.execute(
                    "UPDATE cards 
                     SET box = ?1, next_review = ?2, attempts = ?3 
                     WHERE ROWID = ?4",
                    params![
                        new_box,
                        review_date.format("%Y-%m-%d").to_string(),
                        new_attempts,
                        self.selected_index + 1
                    ],
                )?;
                self.review_due[self.selected_index] = review_date;
                self.box_level[self.selected_index] = new_box;
            }
            Ok(())
        } else {
            Err(rusqlite::Error::InvalidQuery)
        }
    }
}

pub fn get_box_symbol(box_num: u8) -> String {
    match box_num {
        1 => "★☆☆☆☆".to_string(),
        2 => "★★☆☆☆".to_string(),
        3 => "★★★☆☆".to_string(),
        4 => "★★★★☆".to_string(),
        5 => "★★★★★".to_string(),
        _ => "☆☆☆☆☆".to_string(),
    }
}

pub fn get_relative_date(date: NaiveDate) -> String {
    let today = chrono::Local::now().date_naive();
    let tomorrow = today + chrono::Duration::days(1);

    if date < today {
        return "".to_string();
    } else if date == today {
        return "Today".to_string();
    } else if date == tomorrow {
        return "Tomorrow".to_string();
    } else if date.iso_week() == today.iso_week() {
        return format!("{}", date.weekday());
    } else if date.signed_duration_since(today).num_days() <= 7 {
        return "Next week".to_string();
    } else if date.signed_duration_since(today).num_days() <= 10 {
        return format!("In {} days", date.signed_duration_since(today).num_days());
    }
    "".to_string()
}
