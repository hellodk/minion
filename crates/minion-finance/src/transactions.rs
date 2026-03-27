//! Transaction tracking

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{Error, Result, TransactionType};

/// A financial transaction.
#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: String,
    pub account_id: String,
    pub transaction_type: TransactionType,
    pub amount: f64,
    pub description: String,
    pub category: Option<String>,
    pub date: DateTime<Utc>,
    pub tags: Vec<String>,
}

/// Manages a collection of financial transactions in memory.
pub struct TransactionManager {
    transactions: Vec<Transaction>,
}

impl TransactionManager {
    /// Creates a new empty transaction manager.
    pub fn new() -> Self {
        Self {
            transactions: Vec::new(),
        }
    }

    /// Adds a new transaction. Amount must be positive.
    pub fn add(
        &mut self,
        account_id: &str,
        transaction_type: TransactionType,
        amount: f64,
        description: &str,
        category: Option<&str>,
        tags: Vec<String>,
    ) -> Result<Transaction> {
        if amount <= 0.0 {
            return Err(Error::Transaction("Amount must be positive".to_string()));
        }
        if description.is_empty() {
            return Err(Error::Transaction(
                "Description cannot be empty".to_string(),
            ));
        }

        let tx = Transaction {
            id: Uuid::new_v4().to_string(),
            account_id: account_id.to_string(),
            transaction_type,
            amount,
            description: description.to_string(),
            category: category.map(|c| c.to_string()),
            date: Utc::now(),
            tags,
        };

        self.transactions.push(tx.clone());
        Ok(tx)
    }

    /// Returns a slice of all transactions.
    pub fn list(&self) -> &[Transaction] {
        &self.transactions
    }

    /// Returns transactions for a specific account.
    pub fn list_by_account(&self, account_id: &str) -> Vec<&Transaction> {
        self.transactions
            .iter()
            .filter(|t| t.account_id == account_id)
            .collect()
    }

    /// Returns transactions matching a given category.
    pub fn list_by_category(&self, category: &str) -> Vec<&Transaction> {
        self.transactions
            .iter()
            .filter(|t| t.category.as_deref() == Some(category))
            .collect()
    }

    /// Returns the total amount of all credit (income) transactions.
    pub fn total_income(&self) -> f64 {
        self.transactions
            .iter()
            .filter(|t| t.transaction_type == TransactionType::Credit)
            .map(|t| t.amount)
            .sum()
    }

    /// Returns the total amount of all debit (expense) transactions.
    pub fn total_expenses(&self) -> f64 {
        self.transactions
            .iter()
            .filter(|t| t.transaction_type == TransactionType::Debit)
            .map(|t| t.amount)
            .sum()
    }

    /// Returns transactions within a date range (inclusive).
    pub fn list_by_date_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<&Transaction> {
        self.transactions
            .iter()
            .filter(|t| t.date >= start && t.date <= end)
            .collect()
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager_with_data() -> TransactionManager {
        let mut mgr = TransactionManager::new();
        mgr.add(
            "acct-1",
            TransactionType::Credit,
            3000.0,
            "Salary",
            Some("Income"),
            vec!["recurring".to_string()],
        )
        .unwrap();
        mgr.add(
            "acct-1",
            TransactionType::Debit,
            50.0,
            "Groceries",
            Some("Food"),
            vec![],
        )
        .unwrap();
        mgr.add(
            "acct-2",
            TransactionType::Debit,
            120.0,
            "Electric bill",
            Some("Utilities"),
            vec!["monthly".to_string()],
        )
        .unwrap();
        mgr.add(
            "acct-1",
            TransactionType::Debit,
            30.0,
            "Restaurant",
            Some("Food"),
            vec![],
        )
        .unwrap();
        mgr
    }

    #[test]
    fn test_add_transaction() {
        let mut mgr = TransactionManager::new();
        let tx = mgr
            .add(
                "acct-1",
                TransactionType::Credit,
                100.0,
                "Test",
                None,
                vec![],
            )
            .unwrap();

        assert_eq!(tx.account_id, "acct-1");
        assert_eq!(tx.transaction_type, TransactionType::Credit);
        assert!((tx.amount - 100.0).abs() < f64::EPSILON);
        assert_eq!(tx.description, "Test");
        assert!(tx.category.is_none());
    }

    #[test]
    fn test_add_transaction_invalid_amount() {
        let mut mgr = TransactionManager::new();
        let result = mgr.add("acct-1", TransactionType::Debit, 0.0, "Bad", None, vec![]);
        assert!(result.is_err());

        let result = mgr.add("acct-1", TransactionType::Debit, -10.0, "Bad", None, vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_transaction_empty_description() {
        let mut mgr = TransactionManager::new();
        let result = mgr.add("acct-1", TransactionType::Debit, 10.0, "", None, vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_list() {
        let mgr = make_manager_with_data();
        assert_eq!(mgr.list().len(), 4);
    }

    #[test]
    fn test_list_by_account() {
        let mgr = make_manager_with_data();
        assert_eq!(mgr.list_by_account("acct-1").len(), 3);
        assert_eq!(mgr.list_by_account("acct-2").len(), 1);
        assert!(mgr.list_by_account("nonexistent").is_empty());
    }

    #[test]
    fn test_list_by_category() {
        let mgr = make_manager_with_data();
        assert_eq!(mgr.list_by_category("Food").len(), 2);
        assert_eq!(mgr.list_by_category("Utilities").len(), 1);
        assert_eq!(mgr.list_by_category("Income").len(), 1);
        assert!(mgr.list_by_category("Unknown").is_empty());
    }

    #[test]
    fn test_total_income() {
        let mgr = make_manager_with_data();
        assert!((mgr.total_income() - 3000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_total_expenses() {
        let mgr = make_manager_with_data();
        assert!((mgr.total_expenses() - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_list_by_date_range() {
        let mgr = make_manager_with_data();
        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);

        let in_range = mgr.list_by_date_range(start, end);
        assert_eq!(in_range.len(), 4);

        // Future range should return nothing
        let future_start = Utc::now() + chrono::Duration::hours(1);
        let future_end = Utc::now() + chrono::Duration::hours(2);
        let empty = mgr.list_by_date_range(future_start, future_end);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_default_impl() {
        let mgr = TransactionManager::default();
        assert!(mgr.list().is_empty());
    }
}
