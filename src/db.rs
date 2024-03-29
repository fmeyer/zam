use chrono::{DateTime, Utc};
use csv::Writer;
use rusqlite::{params, Connection, Result};
use std::{fs, io::Cursor};

use crate::alias::Alias;
use crate::alias::AliasDisplay;

const SCHEMA: &'static str = "CREATE TABLE IF NOT EXISTS aliases (
                alias TEXT PRIMARY KEY,
                command TEXT NOT NULL,
                description TEXT NOT NULL,
                date_created TEXT NOT NULL,
                date_updated TEXT NOT NULL
            )";

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new(db_path: String) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        conn.execute(SCHEMA, [])?;

        Ok(Self { conn })
    }

    pub fn add_alias(&self, alias: &Alias) -> Result<()> {
        self.conn.execute(
            "INSERT INTO aliases (alias, command, description, date_created, date_updated)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                alias.alias,
                alias.command,
                alias.description,
                alias.date_created.to_rfc3339(),
                alias.date_updated.to_rfc3339()
            ],
        )?;

        Ok(())
    }
    pub fn update_alias(&self, alias: &Alias) -> Result<()> {
        self.conn.execute(
            "UPDATE aliases SET command = ?1, description = ?2, date_updated = ?3
             WHERE alias = ?4",
            params![
                alias.command,
                alias.description,
                alias.date_updated.to_rfc3339(),
                alias.alias
            ],
        )?;

        Ok(())
    }

    pub fn remove_alias(&self, alias: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM aliases WHERE alias = ?1", params![alias])?;
        Ok(())
    }

    pub fn list_aliases(&self) -> Result<Vec<AliasDisplay>> {
        let mut stmt = self
            .conn
            .prepare("SELECT alias, command, description, date_updated FROM aliases order by alias asc")?;
        let rows = stmt.query_map([], |row| {
            Ok(AliasDisplay {
                alias: row.get::<_, String>(0)?,
                command: row.get::<_, String>(1)?,
                description: row.get::<_, String>(2)?,
                date_updated: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        })?;

        let aliases = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(aliases)
    }

    pub fn export_aliases_to_csv_buffer(&self) -> Result<String, Box<dyn std::error::Error>> {
        let aliases = self.list_aliases()?;

        let mut buffer = Cursor::new(Vec::new());

        // New scope ensures writter drops buffer borrow
        {
            let mut writer = Writer::from_writer(&mut buffer);
            for alias in &aliases {
                writer.serialize(alias)?;
            }
            writer.flush()?;
        }

        let csv_data = String::from_utf8(buffer.into_inner())?;
        Ok(csv_data)
    }

    pub fn export_aliases_to_csv(&self, file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let buffer = self.export_aliases_to_csv_buffer().unwrap();
        fs::write(file_path, buffer)?;
        Ok(())
    }

    pub fn import_aliases_from_csv(
        &self,
        file_path: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut rdr = csv::Reader::from_path(file_path)?;

        for result in rdr.deserialize() {
            let alias: Alias = result?;
            self.add_alias(&alias)?;
        }

        Ok(())
    }
}
