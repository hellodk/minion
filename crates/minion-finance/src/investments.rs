//! Investment portfolio tracking

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{Error, InvestmentType, Result};

/// A single investment holding.
#[derive(Debug, Clone)]
pub struct Investment {
    pub id: String,
    pub name: String,
    pub investment_type: InvestmentType,
    pub purchase_price: f64,
    pub current_price: f64,
    pub quantity: f64,
    pub purchase_date: DateTime<Utc>,
}

/// Manages an investment portfolio in memory.
pub struct InvestmentPortfolio {
    investments: Vec<Investment>,
}

impl InvestmentPortfolio {
    /// Creates a new empty portfolio.
    pub fn new() -> Self {
        Self {
            investments: Vec::new(),
        }
    }

    /// Adds a new investment to the portfolio.
    pub fn add(
        &mut self,
        name: &str,
        investment_type: InvestmentType,
        purchase_price: f64,
        current_price: f64,
        quantity: f64,
    ) -> Result<Investment> {
        if name.is_empty() {
            return Err(Error::Account(
                "Investment name cannot be empty".to_string(),
            ));
        }
        if purchase_price < 0.0 {
            return Err(Error::Account(
                "Purchase price cannot be negative".to_string(),
            ));
        }
        if current_price < 0.0 {
            return Err(Error::Account(
                "Current price cannot be negative".to_string(),
            ));
        }
        if quantity <= 0.0 {
            return Err(Error::Account("Quantity must be positive".to_string()));
        }

        let inv = Investment {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            investment_type,
            purchase_price,
            current_price,
            quantity,
            purchase_date: Utc::now(),
        };

        self.investments.push(inv.clone());
        Ok(inv)
    }

    /// Returns a reference to the investment with the given ID.
    pub fn get(&self, id: &str) -> Option<&Investment> {
        self.investments.iter().find(|i| i.id == id)
    }

    /// Returns a slice of all investments.
    pub fn list(&self) -> &[Investment] {
        &self.investments
    }

    /// Returns the total current market value of the portfolio.
    pub fn total_value(&self) -> f64 {
        self.investments
            .iter()
            .map(|i| i.current_price * i.quantity)
            .sum()
    }

    /// Returns the total cost basis of the portfolio.
    pub fn total_cost(&self) -> f64 {
        self.investments
            .iter()
            .map(|i| i.purchase_price * i.quantity)
            .sum()
    }

    /// Returns the total gain or loss (current value minus cost).
    pub fn total_gain_loss(&self) -> f64 {
        self.total_value() - self.total_cost()
    }

    /// Returns the gain/loss as a percentage of total cost.
    /// Returns 0.0 if the total cost is zero.
    pub fn gain_loss_percent(&self) -> f64 {
        let cost = self.total_cost();
        if cost.abs() < f64::EPSILON {
            return 0.0;
        }
        (self.total_gain_loss() / cost) * 100.0
    }
}

impl Default for InvestmentPortfolio {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_investment() {
        let mut portfolio = InvestmentPortfolio::new();
        let inv = portfolio
            .add("AAPL", InvestmentType::Stock, 150.0, 175.0, 10.0)
            .unwrap();

        assert_eq!(inv.name, "AAPL");
        assert_eq!(inv.investment_type, InvestmentType::Stock);
        assert!((inv.purchase_price - 150.0).abs() < f64::EPSILON);
        assert!((inv.current_price - 175.0).abs() < f64::EPSILON);
        assert!((inv.quantity - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_add_investment_validation() {
        let mut portfolio = InvestmentPortfolio::new();

        assert!(portfolio
            .add("", InvestmentType::Stock, 100.0, 100.0, 1.0)
            .is_err());
        assert!(portfolio
            .add("X", InvestmentType::Stock, -1.0, 100.0, 1.0)
            .is_err());
        assert!(portfolio
            .add("X", InvestmentType::Stock, 100.0, -1.0, 1.0)
            .is_err());
        assert!(portfolio
            .add("X", InvestmentType::Stock, 100.0, 100.0, 0.0)
            .is_err());
    }

    #[test]
    fn test_get_investment() {
        let mut portfolio = InvestmentPortfolio::new();
        let inv = portfolio
            .add("BTC", InvestmentType::Crypto, 30000.0, 45000.0, 0.5)
            .unwrap();

        assert!(portfolio.get(&inv.id).is_some());
        assert!(portfolio.get("nonexistent").is_none());
    }

    #[test]
    fn test_list_investments() {
        let mut portfolio = InvestmentPortfolio::new();
        assert!(portfolio.list().is_empty());

        portfolio
            .add("AAPL", InvestmentType::Stock, 150.0, 175.0, 10.0)
            .unwrap();
        portfolio
            .add("VTI", InvestmentType::Etf, 200.0, 220.0, 5.0)
            .unwrap();
        assert_eq!(portfolio.list().len(), 2);
    }

    #[test]
    fn test_total_value() {
        let mut portfolio = InvestmentPortfolio::new();
        portfolio
            .add("AAPL", InvestmentType::Stock, 150.0, 175.0, 10.0)
            .unwrap();
        portfolio
            .add("VTI", InvestmentType::Etf, 200.0, 220.0, 5.0)
            .unwrap();

        // 175*10 + 220*5 = 1750 + 1100 = 2850
        assert!((portfolio.total_value() - 2850.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_total_cost() {
        let mut portfolio = InvestmentPortfolio::new();
        portfolio
            .add("AAPL", InvestmentType::Stock, 150.0, 175.0, 10.0)
            .unwrap();
        portfolio
            .add("VTI", InvestmentType::Etf, 200.0, 220.0, 5.0)
            .unwrap();

        // 150*10 + 200*5 = 1500 + 1000 = 2500
        assert!((portfolio.total_cost() - 2500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_total_gain_loss() {
        let mut portfolio = InvestmentPortfolio::new();
        portfolio
            .add("AAPL", InvestmentType::Stock, 150.0, 175.0, 10.0)
            .unwrap();
        portfolio
            .add("VTI", InvestmentType::Etf, 200.0, 220.0, 5.0)
            .unwrap();

        // 2850 - 2500 = 350
        assert!((portfolio.total_gain_loss() - 350.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_gain_loss_percent() {
        let mut portfolio = InvestmentPortfolio::new();
        portfolio
            .add("AAPL", InvestmentType::Stock, 150.0, 175.0, 10.0)
            .unwrap();
        portfolio
            .add("VTI", InvestmentType::Etf, 200.0, 220.0, 5.0)
            .unwrap();

        // (350 / 2500) * 100 = 14.0%
        assert!((portfolio.gain_loss_percent() - 14.0).abs() < 0.01);
    }

    #[test]
    fn test_gain_loss_percent_empty() {
        let portfolio = InvestmentPortfolio::new();
        assert!((portfolio.gain_loss_percent() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_negative_gain_loss() {
        let mut portfolio = InvestmentPortfolio::new();
        portfolio
            .add("BAD", InvestmentType::Stock, 100.0, 50.0, 10.0)
            .unwrap();

        // cost: 1000, value: 500, loss: -500
        assert!((portfolio.total_gain_loss() - (-500.0)).abs() < f64::EPSILON);
        assert!((portfolio.gain_loss_percent() - (-50.0)).abs() < 0.01);
    }

    #[test]
    fn test_default_impl() {
        let portfolio = InvestmentPortfolio::default();
        assert!(portfolio.list().is_empty());
    }
}
