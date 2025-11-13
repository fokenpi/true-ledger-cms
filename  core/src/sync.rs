//! CRDT-based synchronization layer for offline-first ledger sync
use std::collections::HashMap;
use automerge::{AutoCommit, ObjId, ObjType, ReadDoc, Value};
use rust_decimal::Decimal;
use uuid::Uuid;
use serde::{Serialize, Deserialize};

use crate::ledger::{Account, AccountType, Transaction};

/// Represents a syncable ledger state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncableLedger {
    pub accounts: HashMap<Uuid, Account>,
    pub transactions: Vec<Transaction>,
    pub balances: HashMap<Uuid, Decimal>,
}

impl SyncableLedger {
    /// Create new empty ledger
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            transactions: Vec::new(),
            balances: HashMap::new(),
        }
    }

    /// Add account to ledger
    pub fn add_account(&mut self, account: Account) {
        self.accounts.insert(account.id, account);
        self.balances.entry(account.id).or_insert(Decimal::ZERO);
    }

    /// Record transaction (assumes already validated)
    pub fn record_transaction(&mut self, tx: Transaction) {
        for posting in &tx.postings {
            *self.balances.entry(posting.account_id).or_insert(Decimal::ZERO) += posting.amount;
        }
        self.transactions.push(tx);
    }
}

/// CRDT document for ledger synchronization
#[derive(Debug, Clone)]
pub struct SyncDoc {
    pub doc: AutoCommit,
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("Automerge error: {0}")]
    Automerge(#[from] automerge::AutomergeError),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Missing required field: {0}")]
    MissingField(&'static str),
}

impl SyncDoc {
    /// Create new sync document with initialized ledger structure
    pub fn new() -> Result<Self, SyncError> {
        let mut doc = AutoCommit::new();
        
        // Initialize ledger structure: { ledger: { accounts: [], transactions: [], balances: {} } }
        let ledger_obj = doc.put_object(&automerge::ROOT, "ledger", ObjType::Map)?;
        doc.put_object(&ledger_obj, "accounts", ObjType::List)?;
        doc.put_object(&ledger_obj, "transactions", ObjType::List)?;
        doc.put_object(&ledger_obj, "balances", ObjType::Map)?;
        
        Ok(Self { doc })
    }

    /// Load sync document from bytes (e.g., received from network)
    pub fn from_bytes( &[u8]) -> Result<Self, SyncError> {
        let doc = AutoCommit::load(data)?;
        Ok(Self { doc })
    }

    /// Serialize document to bytes for network transmission
    pub fn to_bytes(&self) -> Vec<u8> {
        self.doc.save()
    }

    /// Apply local ledger changes to CRDT document
    pub fn update_from_ledger(&mut self, ledger: &SyncableLedger) -> Result<(), SyncError> {
        let ledger_obj = self.get_ledger_obj()?;
        
        // Update accounts
        self.update_accounts(&ledger_obj, &ledger.accounts)?;
        
        // Update transactions
        self.update_transactions(&ledger_obj, &ledger.transactions)?;
        
        // Update balances
        self.update_balances(&ledger_obj, &ledger.balances)?;
        
        Ok(())
    }

    /// Extract ledger state from CRDT document
    pub fn to_ledger(&self) -> Result<SyncableLedger, SyncError> {
        let ledger_obj = self.get_ledger_obj()?;
        
        let accounts = self.read_accounts(&ledger_obj)?;
        let transactions = self.read_transactions(&ledger_obj)?;
        let balances = self.read_balances(&ledger_obj)?;
        
        Ok(SyncableLedger {
            accounts,
            transactions,
            balances,
        })
    }

    /// Merge another sync document (e.g., from peer)
    pub fn merge(&mut self, other: &SyncDoc) -> Result<(), SyncError> {
        self.doc.merge(&other.doc)?;
        Ok(())
    }

    /// Get ledger object ID (root.ledger)
    fn get_ledger_obj(&self) -> Result<ObjId, SyncError> {
        self.doc
            .get(&automerge::ROOT, "ledger")
            .map_err(|_| SyncError::MissingField("ledger"))?
            .and_then(|v| v.cast::<ObjId>())
            .ok_or(SyncError::MissingField("ledger object"))
    }

    /// Update accounts list in CRDT
    fn update_accounts(
        &mut self,
        ledger_obj: &ObjId,
        accounts: &HashMap<Uuid, Account>,
    ) -> Result<(), SyncError> {
        let accounts_list = self.doc
            .get(ledger_obj, "accounts")
            .map_err(|_| SyncError::MissingField("accounts"))?
            .and_then(|v| v.cast::<ObjId>())
            .ok_or(SyncError::MissingField("accounts list"))?;

        // Clear and rebuild accounts list
        self.doc.clear_list(&accounts_list)?;

        for account in accounts.values() {
            let acc_obj = self.doc.insert_object(&accounts_list, ObjType::Map)?;
            self.doc.put(&acc_obj, "id", account.id.to_string())?;
            self.doc.put(&acc_obj, "name", &account.name)?;
            self.doc.put(&acc_obj, "type", format!("{:?}", account.account_type))?;
        }

        Ok(())
    }

    /// Update transactions list in CRDT
    fn update_transactions(
        &mut self,
        ledger_obj: &ObjId,
        transactions: &[Transaction],
    ) -> Result<(), SyncError> {
        let tx_list = self.doc
            .get(ledger_obj, "transactions")
            .map_err(|_| SyncError::MissingField("transactions"))?
            .and_then(|v| v.cast::<ObjId>())
            .ok_or(SyncError::MissingField("transactions list"))?;

        self.doc.clear_list(&tx_list)?;

        for tx in transactions {
            let tx_obj = self.doc.insert_object(&tx_list, ObjType::Map)?;
            self.doc.put(&tx_obj, "id", tx.id.to_string())?;
            self.doc.put(&tx_obj, "date", tx.date.to_string())?;
            self.doc.put(&tx_obj, "description", &tx.description)?;
            
            // Serialize postings as JSON array
            let postings_json = serde_json::to_string(&tx.postings)?;
            self.doc.put(&tx_obj, "postings", postings_json)?;
        }

        Ok(())
    }

    /// Update balances map in CRDT
    fn update_balances(
        &mut self,
        ledger_obj: &ObjId,
        balances: &HashMap<Uuid, Decimal>,
    ) -> Result<(), SyncError> {
        let balances_obj = self.doc
            .get(ledger_obj, "balances")
            .map_err(|_| SyncError::MissingField("balances"))?
            .and_then(|v| v.cast::<ObjId>())
            .ok_or(SyncError::MissingField("balances map"))?;

        // Clear and rebuild balances
        let keys: Vec<String> = self.doc
            .keys(&balances_obj)
            .map(|k| k.to_string())
            .collect();
        for key in keys {
            self.doc.delete(&balances_obj, &key)?;
        }

        for (id, balance) in balances {
            self.doc.put(&balances_obj, &id.to_string(), balance.to_string())?;
        }

        Ok(())
    }

    /// Read accounts from CRDT
    fn read_accounts(&self, ledger_obj: &ObjId) -> Result<HashMap<Uuid, Account>, SyncError> {
        let accounts_list = self.doc
            .get(ledger_obj, "accounts")
            .map_err(|_| SyncError::MissingField("accounts"))?
            .and_then(|v| v.cast::<ObjId>())
            .ok_or(SyncError::MissingField("accounts list"))?;

        let mut accounts = HashMap::new();
        for i in 0..self.doc.length(&accounts_list) {
            if let Some(Value::Object(ObjType::Map, acc_obj)) = self.doc.get(&accounts_list, i)? {
                let id_str: String = self.doc
                    .get(&acc_obj, "id")?
                    .and_then(|v| v.cast::<String>())
                    .ok_or(SyncError::MissingField("account.id"))?;
                let id = Uuid::parse_str(&id_str).map_err(|_| SyncError::MissingField("invalid UUID"))?;

                let name: String = self.doc
                    .get(&acc_obj, "name")?
                    .and_then(|v| v.cast::<String>())
                    .ok_or(SyncError::MissingField("account.name"))?;

                let type_str: String = self.doc
                    .get(&acc_obj, "type")?
                    .and_then(|v| v.cast::<String>())
                    .ok_or(SyncError::MissingField("account.type"))?;
                let account_type = match type_str.as_str() {
                    "Asset" => AccountType::Asset,
                    "Liability" => AccountType::Liability,
                    "Equity" => AccountType::Equity,
                    "Revenue" => AccountType::Revenue,
                    "Expense" => AccountType::Expense,
                    _ => return Err(SyncError::MissingField("unknown account type")),
                };

                accounts.insert(id, Account {
                    id,
                    name,
                    account_type,
                    parent_id: None,
                });
            }
        }

        Ok(accounts)
    }

    /// Read transactions from CRDT
    fn read_transactions(&self, ledger_obj: &ObjId) -> Result<Vec<Transaction>, SyncError> {
        let tx_list = self.doc
            .get(ledger_obj, "transactions")
            .map_err(|_| SyncError::MissingField("transactions"))?
            .and_then(|v| v.cast::<ObjId>())
            .ok_or(SyncError::MissingField("transactions list"))?;

        let mut transactions = Vec::new();
        for i in 0..self.doc.length(&tx_list) {
            if let Some(Value::Object(ObjType::Map, tx_obj)) = self.doc.get(&tx_list, i)? {
                let id_str: String = self.doc
                    .get(&tx_obj, "id")?
                    .and_then(|v| v.cast::<String>())
                    .ok_or(SyncError::MissingField("transaction.id"))?;
                let id = Uuid::parse_str(&id_str).map_err(|_| SyncError::MissingField("invalid UUID"))?;

                let date_str: String = self.doc
                    .get(&tx_obj, "date")?
                    .and_then(|v| v.cast::<String>())
                    .ok_or(SyncError::MissingField("transaction.date"))?;
                let date = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                    .map_err(|_| SyncError::MissingField("invalid date format"))?;

                let description: String = self.doc
                    .get(&tx_obj, "description")?
                    .and_then(|v| v.cast::<String>())
                    .ok_or(SyncError::MissingField("transaction.description"))?;

                let postings_json: String = self.doc
                    .get(&tx_obj, "postings")?
                    .and_then(|v| v.cast::<String>())
                    .ok_or(SyncError::MissingField("transaction.postings"))?;
                let postings: Vec<super::ledger::Posting> = serde_json::from_str(&postings_json)?;

                transactions.push(Transaction {
                    id,
                    date,
                    description,
                    postings,
                    is_closing_entry: false,
                    is_reversing_entry: false,
                    meta Default::default(),
                });
            }
        }

        Ok(transactions)
    }

    /// Read balances from CRDT
    fn read_balances(&self, ledger_obj: &ObjId)