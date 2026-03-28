//! Bank statement and transaction import

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::{Error, Result};

/// A single transaction parsed from a CSV import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedTransaction {
    pub date: String,
    pub description: String,
    pub amount: f64,
    pub transaction_type: String, // "credit" or "debit"
    pub category: Option<String>,
    pub balance: Option<f64>,
}

/// Summary of a CSV import operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub total_rows: usize,
    pub imported: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
    pub transactions: Vec<ImportedTransaction>,
}

/// Describes how CSV columns map to transaction fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvColumnMapping {
    pub date_column: String,
    pub description_column: String,
    pub amount_column: String,
    pub debit_column: Option<String>,
    pub credit_column: Option<String>,
    pub balance_column: Option<String>,
    pub date_format: String,
}

impl Default for CsvColumnMapping {
    fn default() -> Self {
        Self {
            date_column: "Date".to_string(),
            description_column: "Description".to_string(),
            amount_column: "Amount".to_string(),
            debit_column: None,
            credit_column: None,
            balance_column: None,
            date_format: "%d/%m/%Y".to_string(),
        }
    }
}

/// Import transactions from a CSV file using the given column mapping.
///
/// Rows that cannot be parsed are recorded in `ImportResult::errors` and
/// counted as `skipped`; the remaining rows are returned as
/// `ImportedTransaction` values with auto-detected categories.
pub fn import_csv(path: &Path, mapping: &CsvColumnMapping) -> Result<ImportResult> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_path(path)
        .map_err(|e| Error::Import(format!("Failed to open CSV: {e}")))?;

    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| Error::Import(format!("Failed to read CSV headers: {e}")))?
        .iter()
        .map(|h| h.to_string())
        .collect();

    // Resolve column indices once.
    let date_idx = find_column_index(&headers, &mapping.date_column);
    let desc_idx = find_column_index(&headers, &mapping.description_column);
    let amount_idx = find_column_index(&headers, &mapping.amount_column);
    let debit_idx = mapping
        .debit_column
        .as_ref()
        .and_then(|c| find_column_index(&headers, c));
    let credit_idx = mapping
        .credit_column
        .as_ref()
        .and_then(|c| find_column_index(&headers, c));
    let balance_idx = mapping
        .balance_column
        .as_ref()
        .and_then(|c| find_column_index(&headers, c));

    // We need at least date + description + (amount OR debit/credit).
    if date_idx.is_none() || desc_idx.is_none() {
        return Err(Error::Import(
            "Required columns (date, description) not found in CSV".to_string(),
        ));
    }
    if amount_idx.is_none() && (debit_idx.is_none() || credit_idx.is_none()) {
        return Err(Error::Import(
            "Need either an amount column or both debit and credit columns".to_string(),
        ));
    }

    let date_idx = date_idx.unwrap();
    let desc_idx = desc_idx.unwrap();

    let mut result = ImportResult {
        total_rows: 0,
        imported: 0,
        skipped: 0,
        errors: Vec::new(),
        transactions: Vec::new(),
    };

    for (row_num, record) in reader.records().enumerate() {
        result.total_rows += 1;
        let row_label = row_num + 2; // 1-indexed, header is row 1

        let record = match record {
            Ok(r) => r,
            Err(e) => {
                result.skipped += 1;
                result
                    .errors
                    .push(format!("Row {row_label}: failed to read: {e}"));
                continue;
            }
        };

        let date = match record.get(date_idx) {
            Some(v) if !v.trim().is_empty() => v.trim().to_string(),
            _ => {
                result.skipped += 1;
                result.errors.push(format!("Row {row_label}: missing date"));
                continue;
            }
        };

        let description = match record.get(desc_idx) {
            Some(v) if !v.trim().is_empty() => v.trim().to_string(),
            _ => {
                result.skipped += 1;
                result
                    .errors
                    .push(format!("Row {row_label}: missing description"));
                continue;
            }
        };

        // Determine amount and transaction type.
        let (amount, tx_type) =
            match resolve_amount(&record, amount_idx, debit_idx, credit_idx, row_label) {
                Ok(v) => v,
                Err(msg) => {
                    result.skipped += 1;
                    result.errors.push(msg);
                    continue;
                }
            };

        let balance =
            balance_idx.and_then(|i| record.get(i).and_then(|v| parse_amount_str(v.trim()).ok()));

        let category = Some(auto_categorize(&description));

        result.transactions.push(ImportedTransaction {
            date,
            description,
            amount,
            transaction_type: tx_type,
            category,
            balance,
        });
        result.imported += 1;
    }

    Ok(result)
}

/// Attempt to find a column index by case-insensitive header match.
fn find_column_index(headers: &[String], name: &str) -> Option<usize> {
    let lower = name.to_lowercase();
    headers.iter().position(|h| h.to_lowercase() == lower)
}

/// Parse an amount string, stripping common currency symbols and commas.
fn parse_amount_str(s: &str) -> std::result::Result<f64, String> {
    let cleaned: String = s
        .chars()
        .filter(|c| *c != ',' && *c != '$' && *c != '\u{20b9}' && *c != ' ')
        .collect();
    cleaned
        .parse::<f64>()
        .map_err(|e| format!("cannot parse amount '{s}': {e}"))
}

/// Resolve amount and transaction type from either a single amount column or
/// separate debit/credit columns.
fn resolve_amount(
    record: &csv::StringRecord,
    amount_idx: Option<usize>,
    debit_idx: Option<usize>,
    credit_idx: Option<usize>,
    row_label: usize,
) -> std::result::Result<(f64, String), String> {
    // Prefer separate debit/credit columns when both are present.
    if let (Some(di), Some(ci)) = (debit_idx, credit_idx) {
        let debit_str = record.get(di).unwrap_or("").trim();
        let credit_str = record.get(ci).unwrap_or("").trim();
        let debit_val = if debit_str.is_empty() {
            0.0
        } else {
            parse_amount_str(debit_str)
                .map_err(|e| format!("Row {row_label}: debit {e}"))?
                .abs()
        };
        let credit_val = if credit_str.is_empty() {
            0.0
        } else {
            parse_amount_str(credit_str)
                .map_err(|e| format!("Row {row_label}: credit {e}"))?
                .abs()
        };

        if credit_val > 0.0 {
            return Ok((credit_val, "credit".to_string()));
        }
        if debit_val > 0.0 {
            return Ok((debit_val, "debit".to_string()));
        }
        return Err(format!(
            "Row {row_label}: both debit and credit are zero or empty"
        ));
    }

    // Single amount column.
    if let Some(ai) = amount_idx {
        let raw = record.get(ai).unwrap_or("").trim();
        if raw.is_empty() {
            return Err(format!("Row {row_label}: missing amount"));
        }
        let val = parse_amount_str(raw).map_err(|e| format!("Row {row_label}: {e}"))?;
        if val >= 0.0 {
            Ok((val, "credit".to_string()))
        } else {
            Ok((val.abs(), "debit".to_string()))
        }
    } else {
        Err(format!("Row {row_label}: no amount column available"))
    }
}

/// Attempt to auto-detect column mappings from a set of CSV headers.
///
/// Uses common header name patterns found in bank statements.
pub fn auto_detect_columns(headers: &[String]) -> CsvColumnMapping {
    let lower_headers: Vec<String> = headers.iter().map(|h| h.to_lowercase()).collect();

    let date_column = detect_one(
        headers,
        &lower_headers,
        &["date", "txn date", "transaction date", "value date"],
    )
    .unwrap_or_else(|| "Date".to_string());

    let description_column = detect_one(
        headers,
        &lower_headers,
        &[
            "description",
            "narration",
            "particulars",
            "details",
            "transaction details",
        ],
    )
    .unwrap_or_else(|| "Description".to_string());

    let amount_column = detect_one(
        headers,
        &lower_headers,
        &["amount", "txn amount", "transaction amount"],
    )
    .unwrap_or_else(|| "Amount".to_string());

    let debit_column = detect_one(
        headers,
        &lower_headers,
        &["debit", "withdrawal", "dr", "debit amount"],
    );

    let credit_column = detect_one(
        headers,
        &lower_headers,
        &["credit", "deposit", "cr", "credit amount"],
    );

    let balance_column = detect_one(
        headers,
        &lower_headers,
        &["balance", "closing balance", "available balance"],
    );

    CsvColumnMapping {
        date_column,
        description_column,
        amount_column,
        debit_column,
        credit_column,
        balance_column,
        date_format: "%d/%m/%Y".to_string(),
    }
}

/// Return the original-case header that matches any of the given patterns.
fn detect_one(originals: &[String], lower: &[String], patterns: &[&str]) -> Option<String> {
    for pat in patterns {
        if let Some(idx) = lower.iter().position(|h| h == pat) {
            return Some(originals[idx].clone());
        }
    }
    None
}

/// Rule-based auto-categorisation of a transaction description.
pub fn auto_categorize(description: &str) -> String {
    let lower = description.to_lowercase();

    let rules: &[(&[&str], &str)] = &[
        (
            &["swiggy", "zomato", "uber eats", "dominos", "pizza"],
            "Food & Dining",
        ),
        (
            &["amazon", "flipkart", "myntra", "ajio", "meesho"],
            "Shopping",
        ),
        (
            &["uber", "ola", "rapido", "metro", "irctc", "railway"],
            "Transport",
        ),
        (
            &[
                "netflix",
                "hotstar",
                "spotify",
                "youtube",
                "prime video",
                "disney",
            ],
            "Entertainment",
        ),
        (&["rent", "maintenance", "society", "housing"], "Housing"),
        (
            &[
                "electricity",
                "water",
                "gas",
                "broadband",
                "jio",
                "airtel",
                "vodafone",
                "bsnl",
            ],
            "Utilities",
        ),
        (
            &["hospital", "pharmacy", "medical", "apollo", "1mg"],
            "Healthcare",
        ),
        (
            &["mutual fund", "sip", "groww", "zerodha", "kuvera", "mf"],
            "Investment",
        ),
        (&["salary", "payroll"], "Income"),
        (&["atm", "cash withdrawal"], "Cash Withdrawal"),
        (&["insurance", "lic", "premium", "policy"], "Insurance"),
        (&["emi", "loan", "repayment"], "EMI/Loan"),
    ];

    for (keywords, category) in rules {
        for kw in *keywords {
            if lower.contains(kw) {
                return (*category).to_string();
            }
        }
    }

    "Other".to_string()
}

/// Compound Annual Growth Rate.
///
/// Returns the CAGR as a percentage (e.g. 12.5 means 12.5%).
/// Returns 0.0 when inputs are invalid (non-positive initial value or years).
pub fn calculate_cagr(initial_value: f64, final_value: f64, years: f64) -> f64 {
    if initial_value <= 0.0 || years <= 0.0 {
        return 0.0;
    }
    ((final_value / initial_value).powf(1.0 / years) - 1.0) * 100.0
}

/// Breakdown of net worth into assets, liabilities, and per-type totals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetWorthBreakdown {
    pub assets: f64,
    pub liabilities: f64,
    pub net_worth: f64,
    pub by_type: HashMap<String, f64>,
}

impl NetWorthBreakdown {
    /// Build a net-worth breakdown from a map of `(type_name -> value)`.
    ///
    /// Positive values are treated as assets; negative values as liabilities.
    pub fn from_entries(entries: HashMap<String, f64>) -> Self {
        let mut assets = 0.0;
        let mut liabilities = 0.0;

        for val in entries.values() {
            if *val >= 0.0 {
                assets += val;
            } else {
                liabilities += val.abs();
            }
        }

        Self {
            assets,
            liabilities,
            net_worth: assets - liabilities,
            by_type: entries,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Helper: write CSV content to a temp file and return the path.
    fn write_csv(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    // ---------------------------------------------------------------
    // CSV import tests
    // ---------------------------------------------------------------

    #[test]
    fn test_import_csv_standard_format() {
        let csv = "\
Date,Description,Amount,Balance
01/01/2025,Salary,50000.00,50000.00
02/01/2025,Swiggy Order,-350.00,49650.00
03/01/2025,Amazon Purchase,-1200.00,48450.00
";
        let f = write_csv(csv);
        let mapping = CsvColumnMapping {
            balance_column: Some("Balance".to_string()),
            ..CsvColumnMapping::default()
        };
        let result = import_csv(f.path(), &mapping).unwrap();

        assert_eq!(result.total_rows, 3);
        assert_eq!(result.imported, 3);
        assert_eq!(result.skipped, 0);
        assert!(result.errors.is_empty());

        // First row: positive amount -> credit
        assert_eq!(result.transactions[0].transaction_type, "credit");
        assert!((result.transactions[0].amount - 50000.0).abs() < 0.01);
        assert_eq!(result.transactions[0].category.as_deref(), Some("Income"));

        // Second row: negative amount -> debit
        assert_eq!(result.transactions[1].transaction_type, "debit");
        assert!((result.transactions[1].amount - 350.0).abs() < 0.01);
        assert_eq!(
            result.transactions[1].category.as_deref(),
            Some("Food & Dining")
        );

        // Balance parsed correctly
        assert!((result.transactions[0].balance.unwrap() - 50000.0).abs() < 0.01);
        assert!((result.transactions[1].balance.unwrap() - 49650.0).abs() < 0.01);
    }

    #[test]
    fn test_import_csv_separate_debit_credit_columns() {
        let csv = "\
Date,Description,Debit,Credit,Balance
01/01/2025,Salary,,50000.00,50000.00
02/01/2025,Uber Ride,250.00,,49750.00
";
        let f = write_csv(csv);
        let mapping = CsvColumnMapping {
            date_column: "Date".to_string(),
            description_column: "Description".to_string(),
            amount_column: "Amount".to_string(), // not present; will use debit/credit
            debit_column: Some("Debit".to_string()),
            credit_column: Some("Credit".to_string()),
            balance_column: Some("Balance".to_string()),
            date_format: "%d/%m/%Y".to_string(),
        };

        let result = import_csv(f.path(), &mapping).unwrap();
        assert_eq!(result.imported, 2);
        assert_eq!(result.transactions[0].transaction_type, "credit");
        assert!((result.transactions[0].amount - 50000.0).abs() < 0.01);
        assert_eq!(result.transactions[1].transaction_type, "debit");
        assert!((result.transactions[1].amount - 250.0).abs() < 0.01);
    }

    #[test]
    fn test_import_csv_malformed_rows() {
        let csv = "\
Date,Description,Amount
01/01/2025,Valid,100.00
02/01/2025,,200.00
03/01/2025,Bad Amount,notanumber
04/01/2025,Also Valid,-50.00
";
        let f = write_csv(csv);
        let mapping = CsvColumnMapping::default();
        let result = import_csv(f.path(), &mapping).unwrap();

        assert_eq!(result.total_rows, 4);
        assert_eq!(result.imported, 2);
        assert_eq!(result.skipped, 2);
        assert_eq!(result.errors.len(), 2);
        // Row 3 (empty description) and row 4 (bad amount)
        assert!(result.errors[0].contains("Row 3"));
        assert!(result.errors[1].contains("Row 4"));
    }

    #[test]
    fn test_import_csv_empty_file() {
        let csv = "Date,Description,Amount\n";
        let f = write_csv(csv);
        let mapping = CsvColumnMapping::default();
        let result = import_csv(f.path(), &mapping).unwrap();

        assert_eq!(result.total_rows, 0);
        assert_eq!(result.imported, 0);
        assert!(result.transactions.is_empty());
    }

    #[test]
    fn test_import_csv_missing_required_columns() {
        let csv = "Foo,Bar,Baz\n1,2,3\n";
        let f = write_csv(csv);
        let mapping = CsvColumnMapping::default();
        let result = import_csv(f.path(), &mapping);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Required columns"));
    }

    #[test]
    fn test_import_csv_nonexistent_file() {
        let mapping = CsvColumnMapping::default();
        let result = import_csv(Path::new("/tmp/does_not_exist_12345.csv"), &mapping);
        assert!(result.is_err());
    }

    #[test]
    fn test_import_csv_with_commas_in_amounts() {
        let csv = "\
Date,Description,Amount
01/01/2025,Big Salary,\"50,000.00\"
02/01/2025,Shopping,\"-1,200.50\"
";
        let f = write_csv(csv);
        let mapping = CsvColumnMapping::default();
        let result = import_csv(f.path(), &mapping).unwrap();

        assert_eq!(result.imported, 2);
        assert!((result.transactions[0].amount - 50000.0).abs() < 0.01);
        assert!((result.transactions[1].amount - 1200.50).abs() < 0.01);
    }

    // ---------------------------------------------------------------
    // Auto-detect columns tests
    // ---------------------------------------------------------------

    #[test]
    fn test_auto_detect_standard_headers() {
        let headers: Vec<String> = vec![
            "Date".to_string(),
            "Description".to_string(),
            "Amount".to_string(),
            "Balance".to_string(),
        ];
        let mapping = auto_detect_columns(&headers);
        assert_eq!(mapping.date_column, "Date");
        assert_eq!(mapping.description_column, "Description");
        assert_eq!(mapping.amount_column, "Amount");
        assert_eq!(mapping.balance_column, Some("Balance".to_string()));
        assert!(mapping.debit_column.is_none());
        assert!(mapping.credit_column.is_none());
    }

    #[test]
    fn test_auto_detect_bank_statement_headers() {
        let headers: Vec<String> = vec![
            "Txn Date".to_string(),
            "Narration".to_string(),
            "Debit".to_string(),
            "Credit".to_string(),
            "Closing Balance".to_string(),
        ];
        let mapping = auto_detect_columns(&headers);
        assert_eq!(mapping.date_column, "Txn Date");
        assert_eq!(mapping.description_column, "Narration");
        assert_eq!(mapping.debit_column, Some("Debit".to_string()));
        assert_eq!(mapping.credit_column, Some("Credit".to_string()));
        assert_eq!(mapping.balance_column, Some("Closing Balance".to_string()));
    }

    #[test]
    fn test_auto_detect_no_matching_headers() {
        let headers: Vec<String> = vec!["Col1".to_string(), "Col2".to_string(), "Col3".to_string()];
        let mapping = auto_detect_columns(&headers);
        // Falls back to defaults when nothing matches.
        assert_eq!(mapping.date_column, "Date");
        assert_eq!(mapping.description_column, "Description");
        assert_eq!(mapping.amount_column, "Amount");
    }

    #[test]
    fn test_auto_detect_case_insensitive() {
        let headers: Vec<String> = vec![
            "DATE".to_string(),
            "DESCRIPTION".to_string(),
            "AMOUNT".to_string(),
        ];
        let mapping = auto_detect_columns(&headers);
        assert_eq!(mapping.date_column, "DATE");
        assert_eq!(mapping.description_column, "DESCRIPTION");
        assert_eq!(mapping.amount_column, "AMOUNT");
    }

    // ---------------------------------------------------------------
    // Auto-categorize tests
    // ---------------------------------------------------------------

    #[test]
    fn test_auto_categorize_food() {
        assert_eq!(auto_categorize("Swiggy Order #12345"), "Food & Dining");
        assert_eq!(auto_categorize("ZOMATO*FOOD"), "Food & Dining");
        assert_eq!(auto_categorize("UPI-Uber Eats"), "Food & Dining");
    }

    #[test]
    fn test_auto_categorize_shopping() {
        assert_eq!(auto_categorize("Amazon.in Purchase"), "Shopping");
        assert_eq!(auto_categorize("FLIPKART INTERNET"), "Shopping");
        assert_eq!(auto_categorize("MYNTRA DESIGNS"), "Shopping");
    }

    #[test]
    fn test_auto_categorize_transport() {
        assert_eq!(auto_categorize("Uber Trip"), "Transport");
        assert_eq!(auto_categorize("OLA CABS"), "Transport");
        assert_eq!(auto_categorize("Metro Recharge"), "Transport");
    }

    #[test]
    fn test_auto_categorize_entertainment() {
        assert_eq!(auto_categorize("Netflix Subscription"), "Entertainment");
        assert_eq!(auto_categorize("Spotify Premium"), "Entertainment");
    }

    #[test]
    fn test_auto_categorize_utilities() {
        assert_eq!(auto_categorize("Electricity Bill Payment"), "Utilities");
        assert_eq!(auto_categorize("Jio Prepaid Recharge"), "Utilities");
        assert_eq!(auto_categorize("Airtel Broadband"), "Utilities");
    }

    #[test]
    fn test_auto_categorize_healthcare() {
        assert_eq!(auto_categorize("Apollo Pharmacy"), "Healthcare");
        assert_eq!(auto_categorize("1mg Order"), "Healthcare");
    }

    #[test]
    fn test_auto_categorize_investment() {
        assert_eq!(auto_categorize("SIP Mutual Fund"), "Investment");
        assert_eq!(auto_categorize("Groww Investment"), "Investment");
        assert_eq!(auto_categorize("Zerodha MF Purchase"), "Investment");
    }

    #[test]
    fn test_auto_categorize_income() {
        assert_eq!(auto_categorize("Salary Credit"), "Income");
        assert_eq!(auto_categorize("Monthly Payroll"), "Income");
    }

    #[test]
    fn test_auto_categorize_housing() {
        assert_eq!(auto_categorize("Monthly Rent"), "Housing");
        assert_eq!(auto_categorize("Society Maintenance"), "Housing");
    }

    #[test]
    fn test_auto_categorize_emi() {
        assert_eq!(auto_categorize("Home Loan EMI"), "EMI/Loan");
    }

    #[test]
    fn test_auto_categorize_insurance() {
        assert_eq!(auto_categorize("LIC Premium"), "Insurance");
    }

    #[test]
    fn test_auto_categorize_cash() {
        assert_eq!(auto_categorize("ATM Withdrawal"), "Cash Withdrawal");
    }

    #[test]
    fn test_auto_categorize_unknown() {
        assert_eq!(auto_categorize("Random Transfer XYZ"), "Other");
    }

    // ---------------------------------------------------------------
    // CAGR tests
    // ---------------------------------------------------------------

    #[test]
    fn test_cagr_basic() {
        // $100 growing to $200 in 3 years
        let cagr = calculate_cagr(100.0, 200.0, 3.0);
        // Expected ~26.0%
        assert!((cagr - 26.0).abs() < 0.1);
    }

    #[test]
    fn test_cagr_no_growth() {
        let cagr = calculate_cagr(1000.0, 1000.0, 5.0);
        assert!((cagr - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_cagr_negative_growth() {
        let cagr = calculate_cagr(1000.0, 500.0, 2.0);
        assert!(cagr < 0.0);
    }

    #[test]
    fn test_cagr_one_year() {
        // 100 -> 110 in 1 year = 10%
        let cagr = calculate_cagr(100.0, 110.0, 1.0);
        assert!((cagr - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_cagr_invalid_initial_value() {
        assert!((calculate_cagr(0.0, 100.0, 1.0) - 0.0).abs() < f64::EPSILON);
        assert!((calculate_cagr(-100.0, 200.0, 1.0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cagr_invalid_years() {
        assert!((calculate_cagr(100.0, 200.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((calculate_cagr(100.0, 200.0, -1.0) - 0.0).abs() < f64::EPSILON);
    }

    // ---------------------------------------------------------------
    // Net worth tests
    // ---------------------------------------------------------------

    #[test]
    fn test_net_worth_breakdown() {
        let mut entries = HashMap::new();
        entries.insert("Savings".to_string(), 50000.0);
        entries.insert("Stocks".to_string(), 30000.0);
        entries.insert("Home Loan".to_string(), -200000.0);

        let nw = NetWorthBreakdown::from_entries(entries);
        assert!((nw.assets - 80000.0).abs() < 0.01);
        assert!((nw.liabilities - 200000.0).abs() < 0.01);
        assert!((nw.net_worth - (-120000.0)).abs() < 0.01);
        assert_eq!(nw.by_type.len(), 3);
    }

    #[test]
    fn test_net_worth_all_assets() {
        let mut entries = HashMap::new();
        entries.insert("Cash".to_string(), 10000.0);
        entries.insert("FD".to_string(), 50000.0);

        let nw = NetWorthBreakdown::from_entries(entries);
        assert!((nw.assets - 60000.0).abs() < 0.01);
        assert!((nw.liabilities - 0.0).abs() < 0.01);
        assert!((nw.net_worth - 60000.0).abs() < 0.01);
    }

    #[test]
    fn test_net_worth_empty() {
        let entries = HashMap::new();
        let nw = NetWorthBreakdown::from_entries(entries);
        assert!((nw.assets - 0.0).abs() < 0.01);
        assert!((nw.liabilities - 0.0).abs() < 0.01);
        assert!((nw.net_worth - 0.0).abs() < 0.01);
    }

    // ---------------------------------------------------------------
    // CsvColumnMapping default test
    // ---------------------------------------------------------------

    #[test]
    fn test_csv_column_mapping_default() {
        let mapping = CsvColumnMapping::default();
        assert_eq!(mapping.date_column, "Date");
        assert_eq!(mapping.description_column, "Description");
        assert_eq!(mapping.amount_column, "Amount");
        assert!(mapping.debit_column.is_none());
        assert!(mapping.credit_column.is_none());
        assert!(mapping.balance_column.is_none());
        assert_eq!(mapping.date_format, "%d/%m/%Y");
    }

    // ---------------------------------------------------------------
    // parse_amount_str tests
    // ---------------------------------------------------------------

    #[test]
    fn test_parse_amount_str() {
        assert!((parse_amount_str("1234.56").unwrap() - 1234.56).abs() < 0.001);
        assert!((parse_amount_str("1,234.56").unwrap() - 1234.56).abs() < 0.001);
        assert!((parse_amount_str("$1,234.56").unwrap() - 1234.56).abs() < 0.001);
        assert!((parse_amount_str("-500").unwrap() - (-500.0)).abs() < 0.001);
        assert!(parse_amount_str("abc").is_err());
        assert!(parse_amount_str("").is_err());
    }
}
