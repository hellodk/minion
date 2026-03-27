//! Account management

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{AccountType, Error, Result};

/// A financial account.
#[derive(Debug, Clone)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub account_type: AccountType,
    pub balance: f64,
    pub currency: String,
    pub institution: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Manages a collection of financial accounts in memory.
pub struct AccountManager {
    accounts: Vec<Account>,
}

impl AccountManager {
    /// Creates a new empty account manager.
    pub fn new() -> Self {
        Self {
            accounts: Vec::new(),
        }
    }

    /// Adds a new account with the given name, type, and currency.
    /// Returns the newly created account.
    pub fn add(
        &mut self,
        name: &str,
        account_type: AccountType,
        currency: &str,
    ) -> Result<Account> {
        if name.is_empty() {
            return Err(Error::Account("Account name cannot be empty".to_string()));
        }

        let now = Utc::now();
        let account = Account {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            account_type,
            balance: 0.0,
            currency: currency.to_string(),
            institution: None,
            created_at: now,
            updated_at: now,
        };

        self.accounts.push(account.clone());
        Ok(account)
    }

    /// Returns a reference to the account with the given ID, if it exists.
    pub fn get(&self, id: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }

    /// Returns a slice of all accounts.
    pub fn list(&self) -> &[Account] {
        &self.accounts
    }

    /// Updates the balance for the account with the given ID.
    pub fn update_balance(&mut self, id: &str, balance: f64) -> Result<()> {
        let account = self
            .accounts
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| Error::Account(format!("Account not found: {id}")))?;

        account.balance = balance;
        account.updated_at = Utc::now();
        Ok(())
    }

    /// Deletes the account with the given ID.
    pub fn delete(&mut self, id: &str) -> Result<()> {
        let idx = self
            .accounts
            .iter()
            .position(|a| a.id == id)
            .ok_or_else(|| Error::Account(format!("Account not found: {id}")))?;

        self.accounts.remove(idx);
        Ok(())
    }

    /// Returns the sum of all account balances.
    pub fn total_balance(&self) -> f64 {
        self.accounts.iter().map(|a| a.balance).sum()
    }

    /// Returns all accounts matching the given account type.
    pub fn by_type(&self, account_type: AccountType) -> Vec<&Account> {
        self.accounts
            .iter()
            .filter(|a| a.account_type == account_type)
            .collect()
    }
}

impl Default for AccountManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_account() {
        let mut mgr = AccountManager::new();
        let account = mgr.add("Checking", AccountType::Bank, "USD").unwrap();

        assert_eq!(account.name, "Checking");
        assert_eq!(account.account_type, AccountType::Bank);
        assert_eq!(account.currency, "USD");
        assert!((account.balance - 0.0).abs() < f64::EPSILON);
        assert!(!account.id.is_empty());
    }

    #[test]
    fn test_add_account_empty_name() {
        let mut mgr = AccountManager::new();
        let result = mgr.add("", AccountType::Bank, "USD");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_account() {
        let mut mgr = AccountManager::new();
        let account = mgr.add("Savings", AccountType::Bank, "EUR").unwrap();
        let id = account.id.clone();

        let found = mgr.get(&id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Savings");

        assert!(mgr.get("nonexistent").is_none());
    }

    #[test]
    fn test_list_accounts() {
        let mut mgr = AccountManager::new();
        assert!(mgr.list().is_empty());

        mgr.add("A", AccountType::Bank, "USD").unwrap();
        mgr.add("B", AccountType::CreditCard, "USD").unwrap();
        assert_eq!(mgr.list().len(), 2);
    }

    #[test]
    fn test_update_balance() {
        let mut mgr = AccountManager::new();
        let account = mgr.add("Checking", AccountType::Bank, "USD").unwrap();
        let id = account.id.clone();

        mgr.update_balance(&id, 1500.50).unwrap();
        assert!((mgr.get(&id).unwrap().balance - 1500.50).abs() < f64::EPSILON);

        let result = mgr.update_balance("nonexistent", 100.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_account() {
        let mut mgr = AccountManager::new();
        let account = mgr.add("Temp", AccountType::Wallet, "USD").unwrap();
        let id = account.id.clone();

        assert_eq!(mgr.list().len(), 1);
        mgr.delete(&id).unwrap();
        assert!(mgr.list().is_empty());

        let result = mgr.delete("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_total_balance() {
        let mut mgr = AccountManager::new();
        let a1 = mgr.add("A", AccountType::Bank, "USD").unwrap();
        let a2 = mgr.add("B", AccountType::Investment, "USD").unwrap();

        mgr.update_balance(&a1.id, 1000.0).unwrap();
        mgr.update_balance(&a2.id, 2500.0).unwrap();

        assert!((mgr.total_balance() - 3500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_by_type() {
        let mut mgr = AccountManager::new();
        mgr.add("Bank1", AccountType::Bank, "USD").unwrap();
        mgr.add("Bank2", AccountType::Bank, "EUR").unwrap();
        mgr.add("CC", AccountType::CreditCard, "USD").unwrap();

        let banks = mgr.by_type(AccountType::Bank);
        assert_eq!(banks.len(), 2);

        let cards = mgr.by_type(AccountType::CreditCard);
        assert_eq!(cards.len(), 1);

        let loans = mgr.by_type(AccountType::Loan);
        assert!(loans.is_empty());
    }

    #[test]
    fn test_default_impl() {
        let mgr = AccountManager::default();
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_update_balance_updates_timestamp() {
        let mut mgr = AccountManager::new();
        let account = mgr.add("Checking", AccountType::Bank, "USD").unwrap();
        let id = account.id.clone();
        let created = mgr.get(&id).unwrap().updated_at;

        // Small sleep isn't reliable, but updated_at should be >= created
        mgr.update_balance(&id, 500.0).unwrap();
        let updated = mgr.get(&id).unwrap().updated_at;
        assert!(updated >= created);
    }
}
