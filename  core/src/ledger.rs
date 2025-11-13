use rust_decimal::Decimal;
use uuid::Uuid;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: Uuid,
    pub name: String,
    pub r#type: AccountType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccountType {
    Asset, Liability, Equity, Revenue, Expense,
}

impl AccountType {
    pub fn natural_balance(&self) -> AccountKind {
        match self {
            AccountType::Asset | AccountType::Expense => AccountKind::Debit,
            _ => AccountKind::Credit,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccountKind {
    Debit, Credit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Posting {
    pub account_id: Uuid,
    pub amount: Decimal, // +debit, -credit
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: Uuid,
    pub date: chrono::NaiveDate,
    pub description: String,
    pub postings: Vec<Posting>,
}

impl Transaction {
    pub fn is_balanced(&self) -> bool {
        self.postings.iter().map(|p| p.amount).sum::<Decimal>().is_zero()
    }
}

#[derive(Debug, Clone)]
pub struct Ledger {
    accounts: std::collections::HashMap<Uuid, Account>,
    balances: std::collections::HashMap<Uuid, Decimal>,
}

impl Ledger {
    pub fn new() -> Self {
        Self {
            accounts: std::collections::HashMap::new(),
            balances: std::collections::HashMap::new(),
        }
    }

    pub fn add_account(&mut self, account: Account) {
        self.accounts.insert(account.id, account.clone());
        self.balances.insert(account.id, Decimal::ZERO);
    }

    pub fn record_transaction(&mut self, tx: Transaction) -> Result<(), &'static str> {
        if !tx.is_balanced() {
            return Err("Unbalanced transaction");
        }
        for p in &tx.postings {
            if !self.accounts.contains_key(&p.account_id) {
                return Err("Account not found");
            }
            *self.balances.get_mut(&p.account_id).unwrap() += p.amount;
        }
        Ok(())
    }

    pub fn balance(&self, id: &Uuid) -> Decimal {
        *self.balances.get(id).unwrap_or(&Decimal::ZERO)
    }
}