use rusqlite::{Connection, params};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct StoredTransaction {
    pub id: String,
    pub data: String, // JSON-serialized Transaction
}

pub struct LocalStorage {
    conn: Connection,
}

impl LocalStorage {
    pub fn new() -> Self {
        let conn = Connection::open("ledger.db").unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS transactions (
                id TEXT PRIMARY KEY,
                data TEXT NOT NULL
            )",
            [],
        ).unwrap();
        Self { conn }
    }

    pub fn save_transaction(&self, tx: &StoredTransaction) {
        self.conn.execute(
            "INSERT OR REPLACE INTO transactions (id, data) VALUES (?, ?)",
            params![tx.id, tx.data],
        ).unwrap();
    }

    pub fn get_all_transactions(&self) -> Vec<StoredTransaction> {
        let mut stmt = self.conn.prepare("SELECT id, data FROM transactions").unwrap();
        let tx_iter = stmt.query_map([], |row| {
            Ok(StoredTransaction {
                id: row.get(0)?,
                data: row.get(1)?,
            })
        }).unwrap();
        tx_iter.collect::<Result<Vec<_>, _>>().unwrap()
    }
}