//! Financial analytics

use std::collections::HashMap;

use chrono::Datelike;

use crate::accounts::Account;
use crate::transactions::Transaction;
use crate::{AccountType, FinancialSummary, TransactionType};

/// Monthly aggregated financial data.
#[derive(Debug, Clone)]
pub struct MonthlyData {
    pub year: i32,
    pub month: u32,
    pub income: f64,
    pub expenses: f64,
    pub net: f64,
}

/// Provides financial analytics over accounts and transactions.
pub struct FinancialAnalytics;

impl FinancialAnalytics {
    /// Calculates a high-level financial summary from accounts and transactions.
    ///
    /// Assets include Bank, Investment, and Wallet accounts.
    /// Liabilities include CreditCard and Loan accounts.
    /// Monthly income/expenses are computed from all transactions
    /// (this is a simplification treating all transactions as one period).
    pub fn calculate_summary(
        accounts: &[Account],
        transactions: &[Transaction],
    ) -> FinancialSummary {
        let total_assets: f64 = accounts
            .iter()
            .filter(|a| {
                matches!(
                    a.account_type,
                    AccountType::Bank | AccountType::Investment | AccountType::Wallet
                )
            })
            .map(|a| a.balance)
            .sum();

        let total_liabilities: f64 = accounts
            .iter()
            .filter(|a| matches!(a.account_type, AccountType::CreditCard | AccountType::Loan))
            .map(|a| a.balance.abs())
            .sum();

        let net_worth = total_assets - total_liabilities;

        let monthly_income: f64 = transactions
            .iter()
            .filter(|t| t.transaction_type == TransactionType::Credit)
            .map(|t| t.amount)
            .sum();

        let monthly_expenses: f64 = transactions
            .iter()
            .filter(|t| t.transaction_type == TransactionType::Debit)
            .map(|t| t.amount)
            .sum();

        let savings_rate = if monthly_income > 0.0 {
            (monthly_income - monthly_expenses) / monthly_income
        } else {
            0.0
        };

        FinancialSummary {
            net_worth,
            total_assets,
            total_liabilities,
            monthly_income,
            monthly_expenses,
            savings_rate,
        }
    }

    /// Returns a map of category to total spending (debits only).
    /// Transactions without a category are grouped under "Uncategorized".
    pub fn spending_by_category(transactions: &[Transaction]) -> HashMap<String, f64> {
        let mut map: HashMap<String, f64> = HashMap::new();

        for tx in transactions {
            if tx.transaction_type == TransactionType::Debit {
                let cat = tx
                    .category
                    .as_deref()
                    .unwrap_or("Uncategorized")
                    .to_string();
                *map.entry(cat).or_insert(0.0) += tx.amount;
            }
        }

        map
    }

    /// Aggregates transactions into monthly income/expense summaries,
    /// sorted by year and month ascending.
    pub fn monthly_trend(transactions: &[Transaction]) -> Vec<MonthlyData> {
        let mut map: HashMap<(i32, u32), (f64, f64)> = HashMap::new();

        for tx in transactions {
            let key = (tx.date.year(), tx.date.month());
            let entry = map.entry(key).or_insert((0.0, 0.0));
            match tx.transaction_type {
                TransactionType::Credit => entry.0 += tx.amount,
                TransactionType::Debit => entry.1 += tx.amount,
            }
        }

        let mut result: Vec<MonthlyData> = map
            .into_iter()
            .map(|((year, month), (income, expenses))| MonthlyData {
                year,
                month,
                income,
                expenses,
                net: income - expenses,
            })
            .collect();

        result.sort_by(|a, b| a.year.cmp(&b.year).then_with(|| a.month.cmp(&b.month)));

        result
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::AccountType;

    fn make_account(name: &str, account_type: AccountType, balance: f64) -> Account {
        let now = Utc::now();
        Account {
            id: format!("acct-{name}"),
            name: name.to_string(),
            account_type,
            balance,
            currency: "USD".to_string(),
            institution: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn make_transaction(
        tx_type: TransactionType,
        amount: f64,
        category: Option<&str>,
    ) -> Transaction {
        Transaction {
            id: uuid::Uuid::new_v4().to_string(),
            account_id: "acct-1".to_string(),
            transaction_type: tx_type,
            amount,
            description: "test".to_string(),
            category: category.map(|c| c.to_string()),
            date: Utc::now(),
            tags: vec![],
        }
    }

    #[test]
    fn test_calculate_summary_basic() {
        let accounts = vec![
            make_account("checking", AccountType::Bank, 5000.0),
            make_account("savings", AccountType::Bank, 15000.0),
            make_account("cc", AccountType::CreditCard, 2000.0),
        ];

        let transactions = vec![
            make_transaction(TransactionType::Credit, 5000.0, Some("Income")),
            make_transaction(TransactionType::Debit, 1500.0, Some("Rent")),
            make_transaction(TransactionType::Debit, 500.0, Some("Food")),
        ];

        let summary = FinancialAnalytics::calculate_summary(&accounts, &transactions);

        assert!((summary.total_assets - 20000.0).abs() < 0.01);
        assert!((summary.total_liabilities - 2000.0).abs() < 0.01);
        assert!((summary.net_worth - 18000.0).abs() < 0.01);
        assert!((summary.monthly_income - 5000.0).abs() < 0.01);
        assert!((summary.monthly_expenses - 2000.0).abs() < 0.01);
        // savings_rate = (5000 - 2000) / 5000 = 0.6
        assert!((summary.savings_rate - 0.6).abs() < 0.001);
    }

    #[test]
    fn test_calculate_summary_empty() {
        let summary = FinancialAnalytics::calculate_summary(&[], &[]);

        assert!((summary.net_worth - 0.0).abs() < f64::EPSILON);
        assert!((summary.savings_rate - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calculate_summary_no_income() {
        let transactions = vec![make_transaction(TransactionType::Debit, 100.0, None)];

        let summary = FinancialAnalytics::calculate_summary(&[], &transactions);

        assert!((summary.savings_rate - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_spending_by_category() {
        let transactions = vec![
            make_transaction(TransactionType::Debit, 50.0, Some("Food")),
            make_transaction(TransactionType::Debit, 30.0, Some("Food")),
            make_transaction(TransactionType::Debit, 100.0, Some("Rent")),
            make_transaction(TransactionType::Debit, 25.0, None),
            make_transaction(TransactionType::Credit, 5000.0, Some("Income")),
        ];

        let categories = FinancialAnalytics::spending_by_category(&transactions);

        assert!((categories["Food"] - 80.0).abs() < f64::EPSILON);
        assert!((categories["Rent"] - 100.0).abs() < f64::EPSILON);
        assert!((categories["Uncategorized"] - 25.0).abs() < f64::EPSILON);
        // Credit transactions should not appear
        assert!(!categories.contains_key("Income"));
    }

    #[test]
    fn test_spending_by_category_empty() {
        let categories = FinancialAnalytics::spending_by_category(&[]);
        assert!(categories.is_empty());
    }

    #[test]
    fn test_monthly_trend() {
        let transactions = vec![
            make_transaction(TransactionType::Credit, 3000.0, None),
            make_transaction(TransactionType::Debit, 1000.0, None),
            make_transaction(TransactionType::Debit, 500.0, None),
        ];

        let trend = FinancialAnalytics::monthly_trend(&transactions);

        // All transactions are in the same month (now)
        assert_eq!(trend.len(), 1);
        assert!((trend[0].income - 3000.0).abs() < f64::EPSILON);
        assert!((trend[0].expenses - 1500.0).abs() < f64::EPSILON);
        assert!((trend[0].net - 1500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_monthly_trend_empty() {
        let trend = FinancialAnalytics::monthly_trend(&[]);
        assert!(trend.is_empty());
    }

    #[test]
    fn test_summary_with_investment_and_loan() {
        let accounts = vec![
            make_account("invest", AccountType::Investment, 50000.0),
            make_account("wallet", AccountType::Wallet, 500.0),
            make_account("loan", AccountType::Loan, 10000.0),
        ];

        let summary = FinancialAnalytics::calculate_summary(&accounts, &[]);

        assert!((summary.total_assets - 50500.0).abs() < 0.01);
        assert!((summary.total_liabilities - 10000.0).abs() < 0.01);
        assert!((summary.net_worth - 40500.0).abs() < 0.01);
    }
}
