use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Alias {
    pub alias: String,
    pub command: String,
    pub description: String,
    pub date_created: DateTime<Utc>,
    pub date_updated: DateTime<Utc>,
}


#[derive(Debug, Serialize, Deserialize)]
pub struct AliasDisplay {
    pub alias: String,
    pub command: String,
    pub description: String,
    pub date_updated: DateTime<Utc>,
}

impl Alias {
    pub fn new(alias: String, command: String, description: String) -> Self {
        let now = Utc::now();
        Alias {
            alias,
            command,
            description,
            date_created: now,
            date_updated: now,
        }
    }

    pub fn update(&mut self, command: String) {
        self.command = command;
        self.date_updated = Utc::now();
    }
}
