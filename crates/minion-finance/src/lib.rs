//! MINION Finance Intelligence Module
//!
//! Personal finance tracking, investments, and planning.

pub mod accounts;
pub mod analytics;
pub mod goals;
pub mod investments;
pub mod transactions;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Account error: {0}")]
    Account(String),

    #[error("Transaction error: {0}")]
    Transaction(String),

    #[error("Import error: {0}")]
    Import(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(#[from] minion_db::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Account types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountType {
    Bank,
    CreditCard,
    Investment,
    Loan,
    Wallet,
}

/// Transaction types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionType {
    Credit,
    Debit,
}

/// Investment types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvestmentType {
    Stock,
    MutualFund,
    Etf,
    Bond,
    Crypto,
}

/// Financial summary
#[derive(Debug, Clone)]
pub struct FinancialSummary {
    pub net_worth: f64,
    pub total_assets: f64,
    pub total_liabilities: f64,
    pub monthly_income: f64,
    pub monthly_expenses: f64,
    pub savings_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_type_variants() {
        let types = [
            AccountType::Bank,
            AccountType::CreditCard,
            AccountType::Investment,
            AccountType::Loan,
            AccountType::Wallet,
        ];

        for (i, t1) in types.iter().enumerate() {
            for (j, t2) in types.iter().enumerate() {
                if i == j {
                    assert_eq!(t1, t2);
                } else {
                    assert_ne!(t1, t2);
                }
            }
        }
    }

    #[test]
    fn test_transaction_type_variants() {
        assert_eq!(TransactionType::Credit, TransactionType::Credit);
        assert_eq!(TransactionType::Debit, TransactionType::Debit);
        assert_ne!(TransactionType::Credit, TransactionType::Debit);
    }

    #[test]
    fn test_investment_type_variants() {
        let types = [
            InvestmentType::Stock,
            InvestmentType::MutualFund,
            InvestmentType::Etf,
            InvestmentType::Bond,
            InvestmentType::Crypto,
        ];

        for (i, t1) in types.iter().enumerate() {
            for (j, t2) in types.iter().enumerate() {
                if i == j {
                    assert_eq!(t1, t2);
                } else {
                    assert_ne!(t1, t2);
                }
            }
        }
    }

    #[test]
    fn test_financial_summary_creation() {
        let summary = FinancialSummary {
            net_worth: 100000.0,
            total_assets: 150000.0,
            total_liabilities: 50000.0,
            monthly_income: 5000.0,
            monthly_expenses: 3000.0,
            savings_rate: 0.4,
        };

        assert!((summary.net_worth - 100000.0).abs() < 0.01);
        assert!((summary.total_assets - 150000.0).abs() < 0.01);
        assert!((summary.total_liabilities - 50000.0).abs() < 0.01);
        assert!((summary.monthly_income - 5000.0).abs() < 0.01);
        assert!((summary.monthly_expenses - 3000.0).abs() < 0.01);
        assert!((summary.savings_rate - 0.4).abs() < 0.001);
    }

    #[test]
    fn test_financial_summary_net_worth_calculation() {
        let summary = FinancialSummary {
            net_worth: 100000.0,
            total_assets: 150000.0,
            total_liabilities: 50000.0,
            monthly_income: 0.0,
            monthly_expenses: 0.0,
            savings_rate: 0.0,
        };

        // Verify net worth = assets - liabilities
        assert!(
            (summary.net_worth - (summary.total_assets - summary.total_liabilities)).abs() < 0.01
        );
    }

    #[test]
    fn test_financial_summary_clone() {
        let original = FinancialSummary {
            net_worth: 50000.0,
            total_assets: 75000.0,
            total_liabilities: 25000.0,
            monthly_income: 3000.0,
            monthly_expenses: 2000.0,
            savings_rate: 0.33,
        };

        let cloned = original.clone();

        assert!((cloned.net_worth - original.net_worth).abs() < 0.01);
        assert!((cloned.savings_rate - original.savings_rate).abs() < 0.001);
    }

    #[test]
    fn test_error_variants() {
        let account_err = Error::Account("test".to_string());
        assert!(account_err.to_string().contains("Account error"));

        let transaction_err = Error::Transaction("test".to_string());
        assert!(transaction_err.to_string().contains("Transaction error"));

        let import_err = Error::Import("test".to_string());
        assert!(import_err.to_string().contains("Import error"));
    }

    #[test]
    fn test_result_type() {
        let ok_result: Result<i32> = Ok(42);
        assert_eq!(ok_result.unwrap(), 42);

        let err_result: Result<i32> = Err(Error::Account("test".to_string()));
        assert!(err_result.is_err());
    }

    #[test]
    fn test_account_type_copy() {
        let account_type = AccountType::Bank;
        let copied = account_type;
        assert_eq!(account_type, copied);
    }

    #[test]
    fn test_transaction_type_copy() {
        let tx_type = TransactionType::Credit;
        let copied = tx_type;
        assert_eq!(tx_type, copied);
    }

    #[test]
    fn test_investment_type_copy() {
        let inv_type = InvestmentType::Stock;
        let copied = inv_type;
        assert_eq!(inv_type, copied);
    }
}
