import { Component, createSignal, onMount, Show, For, createMemo } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

// ---------- Types ----------

interface InvestmentResponse {
  id: string;
  name: string;
  investment_type: string;
  symbol: string | null;
  exchange: string | null;
  purchase_price: number;
  current_price: number;
  quantity: number;
  purchase_date: string;
  gain_loss: number;
  gain_loss_pct: number;
  current_value: number;
}

interface PortfolioSummary {
  total_invested: number;
  current_value: number;
  total_gain_loss: number;
  total_gain_loss_pct: number;
  by_type: TypeAllocation[];
}

interface TypeAllocation {
  investment_type: string;
  value: number;
  percentage: number;
}

interface FinancialSummaryResponse {
  net_worth: number;
  total_assets: number;
  total_liabilities: number;
  monthly_income: number;
  monthly_expenses: number;
  savings_rate: number;
  account_count: number;
  transaction_count: number;
}

interface FinanceTransactionResponse {
  id: string;
  account_id: string;
  transaction_type: string;
  amount: number;
  description: string | null;
  category: string | null;
  tags: string | null;
  date: string;
  created_at: string;
}

interface FinanceAccountResponse {
  id: string;
  name: string;
  account_type: string;
  institution: string | null;
  balance: number;
  currency: string;
  created_at: string;
  updated_at: string;
}

interface CibilResponse {
  score: number;
  updated_at: string;
}

// ---------- Helpers ----------

const INVESTMENT_TYPES = ['stock', 'mutual_fund', 'etf', 'bond', 'crypto', 'sip'];

const TYPE_LABELS: Record<string, string> = {
  stock: 'Stock',
  mutual_fund: 'Mutual Fund',
  etf: 'ETF',
  bond: 'Bond',
  crypto: 'Crypto',
  sip: 'SIP',
};

const TYPE_COLORS: Record<string, string> = {
  stock: 'bg-blue-500',
  mutual_fund: 'bg-green-500',
  etf: 'bg-purple-500',
  bond: 'bg-yellow-500',
  crypto: 'bg-orange-500',
  sip: 'bg-teal-500',
};

const CATEGORY_COLORS: Record<string, string> = {
  Food: 'bg-red-500',
  Shopping: 'bg-blue-500',
  Transport: 'bg-yellow-500',
  Entertainment: 'bg-purple-500',
  Bills: 'bg-orange-500',
  Health: 'bg-green-500',
  Education: 'bg-teal-500',
  Rent: 'bg-pink-500',
  Travel: 'bg-indigo-500',
  Groceries: 'bg-lime-500',
  Utilities: 'bg-cyan-500',
  Insurance: 'bg-amber-500',
  Subscriptions: 'bg-violet-500',
  Uncategorized: 'bg-gray-500',
};

function formatCurrency(value: number): string {
  return new Intl.NumberFormat('en-IN', {
    style: 'currency',
    currency: 'INR',
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(value);
}

function formatCompact(value: number): string {
  if (Math.abs(value) >= 10000000) {
    return `${(value / 10000000).toFixed(2)} Cr`;
  }
  if (Math.abs(value) >= 100000) {
    return `${(value / 100000).toFixed(2)} L`;
  }
  if (Math.abs(value) >= 1000) {
    return `${(value / 1000).toFixed(1)} K`;
  }
  return value.toFixed(0);
}

function formatPct(value: number): string {
  return `${value >= 0 ? '+' : ''}${value.toFixed(2)}%`;
}

function gainClass(value: number): string {
  if (value > 0) return 'text-green-600 dark:text-green-400';
  if (value < 0) return 'text-red-600 dark:text-red-400';
  return 'text-gray-500';
}

function cibilColor(score: number): string {
  if (score >= 750) return 'text-green-600 dark:text-green-400';
  if (score >= 650) return 'text-yellow-600 dark:text-yellow-400';
  return 'text-red-600 dark:text-red-400';
}

function cibilBg(score: number): string {
  if (score >= 750) return 'bg-green-500';
  if (score >= 650) return 'bg-yellow-500';
  return 'bg-red-500';
}

function daysUntil(dateStr: string): number {
  const now = new Date();
  // Parse the due date - assume it's the next occurrence
  const parts = dateStr.split('-').map(Number);
  if (parts.length < 3) return 999;
  const due = new Date(parts[0], parts[1] - 1, parts[2]);
  const diff = due.getTime() - now.getTime();
  return Math.ceil(diff / (1000 * 60 * 60 * 24));
}

function currentMonthStr(): string {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}`;
}

function monthLabel(monthStr: string): string {
  const [y, m] = monthStr.split('-');
  const months = [
    'January', 'February', 'March', 'April', 'May', 'June',
    'July', 'August', 'September', 'October', 'November', 'December',
  ];
  return `${months[parseInt(m, 10) - 1]} ${y}`;
}

function prevMonth(monthStr: string): string {
  const [y, m] = monthStr.split('-').map(Number);
  if (m <= 1) return `${y - 1}-12`;
  return `${y}-${String(m - 1).padStart(2, '0')}`;
}

function nextMonth(monthStr: string): string {
  const [y, m] = monthStr.split('-').map(Number);
  if (m >= 12) return `${y + 1}-01`;
  return `${y}-${String(m + 1).padStart(2, '0')}`;
}

// ---------- Component ----------

const Finance: Component = () => {
  type TabId = 'overview' | 'investments' | 'transactions' | 'expenses' | 'creditcards' | 'import' | 'cagr';
  const [activeTab, setActiveTab] = createSignal<TabId>('overview');

  // Data signals
  const [summary, setSummary] = createSignal<FinancialSummaryResponse | null>(null);
  const [portfolio, setPortfolio] = createSignal<PortfolioSummary | null>(null);
  const [investments, setInvestments] = createSignal<InvestmentResponse[]>([]);
  const [transactions, setTransactions] = createSignal<FinanceTransactionResponse[]>([]);
  const [accounts, setAccounts] = createSignal<FinanceAccountResponse[]>([]);
  const [cibil, setCibil] = createSignal<CibilResponse | null>(null);

  // Expenses tab state
  const [expenseMonth, setExpenseMonth] = createSignal(currentMonthStr());
  const [spendingByCategory, setSpendingByCategory] = createSignal<Record<string, number>>({});
  const [monthTransactions, setMonthTransactions] = createSignal<FinanceTransactionResponse[]>([]);

  // Credit card form state
  const [showCcForm, setShowCcForm] = createSignal(false);
  const [ccName, setCcName] = createSignal('');
  const [ccBank, setCcBank] = createSignal('');
  const [ccLast4, setCcLast4] = createSignal('');
  const [ccLimit, setCcLimit] = createSignal('');
  const [ccBillingDate, setCcBillingDate] = createSignal('');
  const [ccDueDate, setCcDueDate] = createSignal('');

  // CIBIL form state
  const [cibilInput, setCibilInput] = createSignal('');
  const [showCibilForm, setShowCibilForm] = createSignal(false);

  // UI state
  const [showAddForm, setShowAddForm] = createSignal(false);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  // Add investment form state
  const [formName, setFormName] = createSignal('');
  const [formType, setFormType] = createSignal('stock');
  const [formSymbol, setFormSymbol] = createSignal('');
  const [formExchange, setFormExchange] = createSignal('');
  const [formBuyPrice, setFormBuyPrice] = createSignal('');
  const [formCurrentPrice, setFormCurrentPrice] = createSignal('');
  const [formQuantity, setFormQuantity] = createSignal('');
  const [formDate, setFormDate] = createSignal(new Date().toISOString().split('T')[0]);

  // Update price state
  const [editingPriceId, setEditingPriceId] = createSignal<string | null>(null);
  const [editPrice, setEditPrice] = createSignal('');

  // CAGR calculator state
  const [cagrInitial, setCagrInitial] = createSignal('');
  const [cagrCurrent, setCagrCurrent] = createSignal('');
  const [cagrYears, setCagrYears] = createSignal('');
  const [cagrResult, setCagrResult] = createSignal<number | null>(null);

  // MF NAV fetch state
  const [mfSchemeCode, setMfSchemeCode] = createSignal('');
  const [mfNav, setMfNav] = createSignal<number | null>(null);
  const [mfLoading, setMfLoading] = createSignal(false);

  // CSV import state
  const [importAccountId, setImportAccountId] = createSignal('');

  // ---------- Computed ----------

  const creditCards = createMemo(() =>
    accounts().filter((a) => a.account_type === 'credit_card')
  );

  const totalCreditLimit = createMemo(() =>
    creditCards().reduce((sum, cc) => {
      // credit_limit is stored in institution field as JSON
      const meta = parseCcMeta(cc.institution);
      return sum + (meta.credit_limit || 0);
    }, 0)
  );

  const totalOutstanding = createMemo(() =>
    creditCards().reduce((sum, cc) => sum + Math.abs(cc.balance), 0)
  );

  const ccUtilization = createMemo(() => {
    const limit = totalCreditLimit();
    if (limit <= 0) return 0;
    return (totalOutstanding() / limit) * 100;
  });

  function parseCcMeta(institution: string | null): {
    bank: string;
    last4: string;
    credit_limit: number;
    billing_date: string;
    due_date: string;
  } {
    if (!institution) {
      return { bank: '', last4: '', credit_limit: 0, billing_date: '', due_date: '' };
    }
    try {
      return JSON.parse(institution);
    } catch {
      return { bank: institution, last4: '', credit_limit: 0, billing_date: '', due_date: '' };
    }
  }

  // ---------- Data loading ----------

  async function loadAll() {
    setLoading(true);
    setError(null);
    try {
      const [sum, port, invs, txns, accts, cibilData] = await Promise.all([
        invoke<FinancialSummaryResponse>('finance_get_summary'),
        invoke<PortfolioSummary>('finance_portfolio_summary'),
        invoke<InvestmentResponse[]>('finance_list_investments'),
        invoke<FinanceTransactionResponse[]>('finance_list_transactions', { limit: 100 }),
        invoke<FinanceAccountResponse[]>('finance_list_accounts'),
        invoke<CibilResponse | null>('finance_get_cibil'),
      ]);
      setSummary(sum);
      setPortfolio(port);
      setInvestments(invs);
      setTransactions(txns);
      setAccounts(accts);
      setCibil(cibilData);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function loadExpenses(month: string) {
    try {
      const [cats, txns] = await Promise.all([
        invoke<Record<string, number>>('finance_spending_by_category', { month }),
        invoke<FinanceTransactionResponse[]>('finance_list_transactions', { limit: 500 }),
      ]);
      setSpendingByCategory(cats);
      // Filter for the selected month's debits
      const prefix = month; // "YYYY-MM"
      const filtered = txns
        .filter(
          (tx) =>
            tx.transaction_type === 'debit' && tx.date.startsWith(prefix)
        )
        .sort((a, b) => b.amount - a.amount);
      setMonthTransactions(filtered);
    } catch (e) {
      setError(String(e));
    }
  }

  onMount(loadAll);

  // ---------- Portfolio CAGR computed ----------

  const portfolioCagr = createMemo(() => {
    const p = portfolio();
    if (!p || p.total_invested <= 0 || p.current_value <= 0) return null;
    const invs = investments();
    if (invs.length === 0) return null;
    const dates = invs.map((i) => new Date(i.purchase_date).getTime()).filter((d) => !isNaN(d));
    if (dates.length === 0) return null;
    const earliest = Math.min(...dates);
    const years = (Date.now() - earliest) / (365.25 * 24 * 60 * 60 * 1000);
    if (years < 0.01) return null;
    return ((p.current_value / p.total_invested) ** (1 / years) - 1) * 100;
  });

  // ---------- Actions ----------

  async function handleAddInvestment(e: Event) {
    e.preventDefault();
    setError(null);
    try {
      await invoke<InvestmentResponse>('finance_add_investment', {
        req: {
          name: formName(),
          investment_type: formType(),
          symbol: formSymbol() || null,
          exchange: formExchange() || null,
          purchase_price: parseFloat(formBuyPrice()),
          current_price: parseFloat(formCurrentPrice()),
          quantity: parseFloat(formQuantity()),
          purchase_date: formDate(),
        },
      });
      setFormName('');
      setFormSymbol('');
      setFormExchange('');
      setFormBuyPrice('');
      setFormCurrentPrice('');
      setFormQuantity('');
      setFormDate(new Date().toISOString().split('T')[0]);
      setShowAddForm(false);
      await loadAll();
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleUpdatePrice(id: string) {
    setError(null);
    try {
      await invoke('finance_update_price', {
        investmentId: id,
        newPrice: parseFloat(editPrice()),
      });
      setEditingPriceId(null);
      setEditPrice('');
      await loadAll();
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleDeleteInvestment(id: string) {
    setError(null);
    try {
      await invoke('finance_delete_investment', { investmentId: id });
      await loadAll();
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleCalcCagr() {
    setError(null);
    setCagrResult(null);
    try {
      const result = await invoke<number>('finance_calc_cagr', {
        initial: parseFloat(cagrInitial()),
        current: parseFloat(cagrCurrent()),
        years: parseFloat(cagrYears()),
      });
      setCagrResult(result);
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleFetchMfNav() {
    setMfLoading(true);
    setMfNav(null);
    setError(null);
    try {
      const nav = await invoke<number>('finance_fetch_mf_nav', {
        schemeCode: mfSchemeCode(),
      });
      setMfNav(nav);
    } catch (e) {
      setError(String(e));
    } finally {
      setMfLoading(false);
    }
  }

  async function handleImportCsv() {
    if (!importAccountId()) {
      setError('Select an account for CSV import');
      return;
    }
    setError(null);
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: false,
        filters: [{ name: 'CSV', extensions: ['csv'] }],
      });
      if (!selected || typeof selected !== 'string') return;
      const path = selected;
      const result = await invoke<{ total_rows: number; imported: number; skipped: number; errors: string[] }>(
        'finance_import_csv',
        { path, accountId: importAccountId() }
      );
      alert(`Imported ${result.imported} of ${result.total_rows} rows (${result.skipped} skipped)`);
      await loadAll();
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleAddCreditCard(e: Event) {
    e.preventDefault();
    setError(null);
    try {
      const meta = JSON.stringify({
        bank: ccBank(),
        last4: ccLast4(),
        credit_limit: parseFloat(ccLimit()) || 0,
        billing_date: ccBillingDate(),
        due_date: ccDueDate(),
      });
      await invoke<FinanceAccountResponse>('finance_add_account', {
        name: ccName(),
        accountType: 'credit_card',
        institution: meta,
        balance: 0,
      });
      setCcName('');
      setCcBank('');
      setCcLast4('');
      setCcLimit('');
      setCcBillingDate('');
      setCcDueDate('');
      setShowCcForm(false);
      await loadAll();
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleSaveCibil() {
    setError(null);
    const score = parseInt(cibilInput(), 10);
    if (isNaN(score) || score < 300 || score > 900) {
      setError('CIBIL score must be between 300 and 900');
      return;
    }
    try {
      await invoke('finance_save_cibil', { score });
      setCibilInput('');
      setShowCibilForm(false);
      const cibilData = await invoke<CibilResponse | null>('finance_get_cibil');
      setCibil(cibilData);
    } catch (e) {
      setError(String(e));
    }
  }

  // ---------- Tabs ----------

  const tabs: { id: TabId; label: string }[] = [
    { id: 'overview', label: 'Overview' },
    { id: 'investments', label: 'Investments' },
    { id: 'transactions', label: 'Transactions' },
    { id: 'expenses', label: 'Expenses' },
    { id: 'creditcards', label: 'Credit Cards' },
    { id: 'import', label: 'Import' },
    { id: 'cagr', label: 'CAGR Calculator' },
  ];

  // ---------- Render ----------

  return (
    <div class="p-6">
      <h1 class="text-2xl font-bold mb-6">Finance Intelligence</h1>

      <Show when={error()}>
        <div class="mb-4 p-3 bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 rounded-lg text-sm">
          {error()}
          <button class="ml-2 underline" onClick={() => setError(null)}>
            dismiss
          </button>
        </div>
      </Show>

      {/* Tabs */}
      <div class="flex border-b border-gray-200 dark:border-gray-700 mb-6 overflow-x-auto">
        <For each={tabs}>
          {(tab) => (
            <button
              class="px-4 py-2 -mb-px border-b-2 transition-colors whitespace-nowrap"
              classList={{
                'border-minion-500 text-minion-600 dark:text-minion-400': activeTab() === tab.id,
                'border-transparent text-gray-500 hover:text-gray-700 dark:hover:text-gray-300':
                  activeTab() !== tab.id,
              }}
              onClick={() => {
                setActiveTab(tab.id);
                if (tab.id === 'expenses') {
                  loadExpenses(expenseMonth());
                }
              }}
            >
              {tab.label}
            </button>
          )}
        </For>
      </div>

      {/* ==================== Overview Tab ==================== */}
      <Show when={activeTab() === 'overview'}>
        {/* Net Worth hero card */}
        <div class="card p-6 mb-6 bg-gradient-to-br from-minion-500/10 to-transparent">
          <div class="flex items-center justify-between mb-2">
            <p class="text-sm font-medium text-gray-500 dark:text-gray-400">Net Worth</p>
            <Show when={summary()}>
              <div class="flex gap-4 text-xs text-gray-500 dark:text-gray-400">
                <span>Assets: {formatCurrency(summary()!.total_assets)}</span>
                <span>Liabilities: {formatCurrency(summary()!.total_liabilities)}</span>
              </div>
            </Show>
          </div>
          <p class={`text-4xl font-bold ${summary() ? gainClass(summary()!.net_worth) : ''}`}>
            {summary() ? formatCurrency(summary()!.net_worth) : '--'}
          </p>
          <Show when={portfolio() && portfolio()!.current_value > 0}>
            <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
              Includes investment portfolio: {formatCurrency(portfolio()!.current_value)}
            </p>
          </Show>
        </div>

        {/* Summary cards grid */}
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
          {/* Monthly Cash Flow */}
          <div class="card p-4">
            <p class="text-sm text-gray-500 dark:text-gray-400 mb-2">Monthly Cash Flow</p>
            <div class="space-y-2">
              <div class="flex justify-between items-center">
                <span class="text-xs text-gray-500">Income</span>
                <span class="text-sm font-medium text-green-600 dark:text-green-400">
                  {summary() ? formatCurrency(summary()!.monthly_income) : '--'}
                </span>
              </div>
              <div class="flex justify-between items-center">
                <span class="text-xs text-gray-500">Expenses</span>
                <span class="text-sm font-medium text-red-600 dark:text-red-400">
                  {summary() ? formatCurrency(summary()!.monthly_expenses) : '--'}
                </span>
              </div>
              <Show when={summary()}>
                <div class="flex w-full h-3 rounded-full overflow-hidden bg-gray-200 dark:bg-gray-700 mt-1">
                  <div
                    class="bg-green-500 h-full"
                    style={{
                      width: summary()!.monthly_income > 0
                        ? `${Math.min(((summary()!.monthly_income - summary()!.monthly_expenses) / summary()!.monthly_income) * 100, 100)}%`
                        : '0%',
                    }}
                  />
                  <div class="bg-red-500 h-full flex-1" />
                </div>
              </Show>
            </div>
          </div>

          {/* Savings Rate */}
          <div class="card p-4">
            <p class="text-sm text-gray-500 dark:text-gray-400">Savings Rate</p>
            <p class="text-2xl font-bold">
              {summary() ? `${summary()!.savings_rate.toFixed(1)}%` : '--%'}
            </p>
            <p class="text-xs text-gray-500 mt-1">
              {summary() && summary()!.savings_rate >= 20
                ? 'On track!'
                : 'Target: 20%+'}
            </p>
          </div>

          {/* Credit Card Utilization */}
          <div class="card p-4">
            <p class="text-sm text-gray-500 dark:text-gray-400 mb-2">Credit Card Utilization</p>
            <Show
              when={creditCards().length > 0}
              fallback={<p class="text-gray-400 text-sm">No credit cards</p>}
            >
              <p class={`text-2xl font-bold ${ccUtilization() > 75 ? 'text-red-600 dark:text-red-400' : ccUtilization() > 30 ? 'text-yellow-600 dark:text-yellow-400' : 'text-green-600 dark:text-green-400'}`}>
                {ccUtilization().toFixed(1)}%
              </p>
              <div class="flex w-full h-2 rounded-full overflow-hidden bg-gray-200 dark:bg-gray-700 mt-2">
                <div
                  class={`h-full rounded-full ${ccUtilization() > 75 ? 'bg-red-500' : ccUtilization() > 30 ? 'bg-yellow-500' : 'bg-green-500'}`}
                  style={{ width: `${Math.min(ccUtilization(), 100)}%` }}
                />
              </div>
              <p class="text-xs text-gray-500 mt-1">
                {formatCompact(totalOutstanding())} / {formatCompact(totalCreditLimit())}
              </p>
            </Show>
          </div>

          {/* CIBIL Score */}
          <div class="card p-4">
            <div class="flex items-center justify-between mb-2">
              <p class="text-sm text-gray-500 dark:text-gray-400">CIBIL Score</p>
              <button
                class="text-xs text-minion-600 dark:text-minion-400 underline"
                onClick={() => setShowCibilForm(!showCibilForm())}
              >
                {showCibilForm() ? 'Cancel' : 'Update'}
              </button>
            </div>
            <Show
              when={cibil()}
              fallback={
                <div>
                  <p class="text-gray-400 text-sm mb-2">Not set</p>
                  <Show when={!showCibilForm()}>
                    <button
                      class="text-xs text-minion-600 dark:text-minion-400 underline"
                      onClick={() => setShowCibilForm(true)}
                    >
                      Add your score
                    </button>
                  </Show>
                </div>
              }
            >
              <div>
                <p class={`text-3xl font-bold ${cibilColor(cibil()!.score)}`}>
                  {cibil()!.score}
                </p>
                {/* Gauge bar */}
                <div class="flex w-full h-2 rounded-full overflow-hidden bg-gray-200 dark:bg-gray-700 mt-2">
                  <div
                    class={`h-full rounded-full ${cibilBg(cibil()!.score)}`}
                    style={{ width: `${((cibil()!.score - 300) / 600) * 100}%` }}
                  />
                </div>
                <div class="flex justify-between text-[10px] text-gray-400 mt-0.5">
                  <span>300</span>
                  <span>600</span>
                  <span>900</span>
                </div>
                <p class="text-xs text-gray-500 mt-1">
                  Updated: {new Date(cibil()!.updated_at).toLocaleDateString('en-IN')}
                </p>
              </div>
            </Show>
            <Show when={showCibilForm()}>
              <div class="mt-2 flex gap-2">
                <input
                  type="number"
                  class="input w-20 text-sm"
                  placeholder="300-900"
                  min="300"
                  max="900"
                  value={cibilInput()}
                  onInput={(e) => setCibilInput(e.currentTarget.value)}
                />
                <button
                  class="btn btn-primary text-xs px-3"
                  disabled={!cibilInput()}
                  onClick={handleSaveCibil}
                >
                  Save
                </button>
              </div>
              <p class="text-[10px] text-gray-400 mt-1">
                Check free at paisabazaar.com or bankbazaar.com
              </p>
            </Show>
          </div>
        </div>

        {/* Portfolio summary cards */}
        <h2 class="text-lg font-semibold mb-4">Investment Portfolio</h2>
        <Show
          when={portfolio() && portfolio()!.total_invested > 0}
          fallback={
            <div class="card p-8 text-center mb-8">
              <p class="text-gray-500 dark:text-gray-400 mb-3">
                No investments yet. Add your first investment to see portfolio analytics.
              </p>
              <button
                class="btn btn-primary"
                onClick={() => {
                  setActiveTab('investments');
                  setShowAddForm(true);
                }}
              >
                Add Investment
              </button>
            </div>
          }
        >
          <div class="grid grid-cols-1 md:grid-cols-4 gap-4 mb-6">
            <div class="card p-4">
              <p class="text-sm text-gray-500 dark:text-gray-400">Total Invested</p>
              <p class="text-2xl font-bold">{formatCurrency(portfolio()!.total_invested)}</p>
            </div>
            <div class="card p-4">
              <p class="text-sm text-gray-500 dark:text-gray-400">Current Value</p>
              <p class="text-2xl font-bold">{formatCurrency(portfolio()!.current_value)}</p>
            </div>
            <div class="card p-4">
              <p class="text-sm text-gray-500 dark:text-gray-400">Total Gain/Loss</p>
              <p class={`text-2xl font-bold ${gainClass(portfolio()!.total_gain_loss)}`}>
                {formatCurrency(portfolio()!.total_gain_loss)}
              </p>
              <p class={`text-sm ${gainClass(portfolio()!.total_gain_loss_pct)}`}>
                {formatPct(portfolio()!.total_gain_loss_pct)}
              </p>
            </div>
            <div class="card p-4">
              <p class="text-sm text-gray-500 dark:text-gray-400">Portfolio CAGR</p>
              <p class={`text-2xl font-bold ${portfolioCagr() !== null ? gainClass(portfolioCagr()!) : ''}`}>
                {portfolioCagr() !== null ? formatPct(portfolioCagr()!) : 'N/A'}
              </p>
            </div>
          </div>

          {/* Allocation bar */}
          <div class="card p-4 mb-8">
            <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-3">
              Allocation by Type
            </h3>
            <div class="flex w-full h-6 rounded-lg overflow-hidden mb-3">
              <For each={portfolio()!.by_type}>
                {(alloc) => (
                  <div
                    class={`${TYPE_COLORS[alloc.investment_type] || 'bg-gray-400'} transition-all`}
                    style={{ width: `${Math.max(alloc.percentage, 1)}%` }}
                    title={`${TYPE_LABELS[alloc.investment_type] || alloc.investment_type}: ${formatCurrency(alloc.value)} (${alloc.percentage.toFixed(1)}%)`}
                  />
                )}
              </For>
            </div>
            <div class="flex flex-wrap gap-4 text-sm">
              <For each={portfolio()!.by_type}>
                {(alloc) => (
                  <div class="flex items-center gap-1.5">
                    <span
                      class={`w-3 h-3 rounded-full ${TYPE_COLORS[alloc.investment_type] || 'bg-gray-400'}`}
                    />
                    <span class="text-gray-700 dark:text-gray-300">
                      {TYPE_LABELS[alloc.investment_type] || alloc.investment_type}
                    </span>
                    <span class="text-gray-500">
                      {formatCurrency(alloc.value)} ({alloc.percentage.toFixed(1)}%)
                    </span>
                  </div>
                )}
              </For>
            </div>
          </div>
        </Show>
      </Show>

      {/* ==================== Investments Tab ==================== */}
      <Show when={activeTab() === 'investments'}>
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-lg font-semibold">Your Investments</h2>
          <div class="flex items-center gap-2">
            {/* MF NAV lookup */}
            <div class="flex items-center gap-1">
              <input
                type="text"
                class="input text-sm w-28"
                placeholder="MF Scheme Code"
                value={mfSchemeCode()}
                onInput={(e) => setMfSchemeCode(e.currentTarget.value)}
              />
              <button
                class="btn btn-secondary text-sm"
                disabled={mfLoading() || !mfSchemeCode()}
                onClick={handleFetchMfNav}
              >
                {mfLoading() ? 'Fetching...' : 'Fetch NAV'}
              </button>
              <Show when={mfNav() !== null}>
                <span class="text-sm font-medium text-green-600 dark:text-green-400">
                  NAV: {mfNav()!.toFixed(4)}
                </span>
              </Show>
            </div>
            <button class="btn btn-primary" onClick={() => setShowAddForm(!showAddForm())}>
              {showAddForm() ? 'Cancel' : '+ Add Investment'}
            </button>
          </div>
        </div>

        {/* Add investment form */}
        <Show when={showAddForm()}>
          <form onSubmit={handleAddInvestment} class="card p-4 mb-6">
            <h3 class="font-medium mb-3">Add Investment</h3>
            <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
              <div>
                <label class="block text-xs text-gray-500 mb-1">Name *</label>
                <input
                  type="text"
                  class="input w-full"
                  required
                  value={formName()}
                  onInput={(e) => setFormName(e.currentTarget.value)}
                  placeholder="e.g. Reliance Industries"
                />
              </div>
              <div>
                <label class="block text-xs text-gray-500 mb-1">Type *</label>
                <select
                  class="input w-full"
                  value={formType()}
                  onChange={(e) => setFormType(e.currentTarget.value)}
                >
                  <For each={INVESTMENT_TYPES}>
                    {(t) => <option value={t}>{TYPE_LABELS[t]}</option>}
                  </For>
                </select>
              </div>
              <div>
                <label class="block text-xs text-gray-500 mb-1">Symbol</label>
                <input
                  type="text"
                  class="input w-full"
                  value={formSymbol()}
                  onInput={(e) => setFormSymbol(e.currentTarget.value)}
                  placeholder="e.g. RELIANCE"
                />
              </div>
              <div>
                <label class="block text-xs text-gray-500 mb-1">Exchange</label>
                <select
                  class="input w-full"
                  value={formExchange()}
                  onChange={(e) => setFormExchange(e.currentTarget.value)}
                >
                  <option value="">--</option>
                  <option value="NSE">NSE</option>
                  <option value="BSE">BSE</option>
                </select>
              </div>
              <div>
                <label class="block text-xs text-gray-500 mb-1">Buy Price *</label>
                <input
                  type="number"
                  step="0.01"
                  class="input w-full"
                  required
                  value={formBuyPrice()}
                  onInput={(e) => setFormBuyPrice(e.currentTarget.value)}
                />
              </div>
              <div>
                <label class="block text-xs text-gray-500 mb-1">Current Price *</label>
                <input
                  type="number"
                  step="0.01"
                  class="input w-full"
                  required
                  value={formCurrentPrice()}
                  onInput={(e) => setFormCurrentPrice(e.currentTarget.value)}
                />
              </div>
              <div>
                <label class="block text-xs text-gray-500 mb-1">Quantity *</label>
                <input
                  type="number"
                  step="0.0001"
                  class="input w-full"
                  required
                  value={formQuantity()}
                  onInput={(e) => setFormQuantity(e.currentTarget.value)}
                />
              </div>
              <div>
                <label class="block text-xs text-gray-500 mb-1">Purchase Date *</label>
                <input
                  type="date"
                  class="input w-full"
                  required
                  value={formDate()}
                  onInput={(e) => setFormDate(e.currentTarget.value)}
                />
              </div>
            </div>
            <div class="mt-3 flex justify-end">
              <button type="submit" class="btn btn-primary">
                Add Investment
              </button>
            </div>
          </form>
        </Show>

        {/* Investments table */}
        <Show
          when={investments().length > 0}
          fallback={
            <div class="card p-8 text-center text-gray-500 dark:text-gray-400">
              No investments yet. Click "Add Investment" to get started.
            </div>
          }
        >
          <div class="overflow-x-auto">
            <table class="w-full text-sm">
              <thead>
                <tr class="border-b border-gray-200 dark:border-gray-700 text-left text-gray-500 dark:text-gray-400">
                  <th class="py-2 px-2 font-medium">Name</th>
                  <th class="py-2 px-2 font-medium">Type</th>
                  <th class="py-2 px-2 font-medium">Symbol</th>
                  <th class="py-2 px-2 font-medium text-right">Qty</th>
                  <th class="py-2 px-2 font-medium text-right">Buy Price</th>
                  <th class="py-2 px-2 font-medium text-right">Current Price</th>
                  <th class="py-2 px-2 font-medium text-right">Value</th>
                  <th class="py-2 px-2 font-medium text-right">Gain/Loss</th>
                  <th class="py-2 px-2 font-medium text-right">Gain %</th>
                  <th class="py-2 px-2 font-medium text-center">Actions</th>
                </tr>
              </thead>
              <tbody>
                <For each={investments()}>
                  {(inv) => (
                    <tr class="border-b border-gray-100 dark:border-gray-800 hover:bg-gray-50 dark:hover:bg-gray-800/50">
                      <td class="py-2 px-2 font-medium">{inv.name}</td>
                      <td class="py-2 px-2">
                        <span
                          class={`inline-block px-2 py-0.5 rounded text-xs text-white ${TYPE_COLORS[inv.investment_type] || 'bg-gray-400'}`}
                        >
                          {TYPE_LABELS[inv.investment_type] || inv.investment_type}
                        </span>
                      </td>
                      <td class="py-2 px-2 text-gray-500">
                        {inv.symbol || '--'}
                        {inv.exchange ? ` (${inv.exchange})` : ''}
                      </td>
                      <td class="py-2 px-2 text-right">{inv.quantity}</td>
                      <td class="py-2 px-2 text-right">{formatCurrency(inv.purchase_price)}</td>
                      <td class="py-2 px-2 text-right">
                        <Show
                          when={editingPriceId() === inv.id}
                          fallback={<span>{formatCurrency(inv.current_price)}</span>}
                        >
                          <div class="flex items-center gap-1 justify-end">
                            <input
                              type="number"
                              step="0.01"
                              class="input w-24 text-sm"
                              value={editPrice()}
                              onInput={(e) => setEditPrice(e.currentTarget.value)}
                              onKeyDown={(e) => {
                                if (e.key === 'Enter') handleUpdatePrice(inv.id);
                                if (e.key === 'Escape') setEditingPriceId(null);
                              }}
                            />
                            <button
                              class="text-green-600 hover:text-green-800 text-xs"
                              onClick={() => handleUpdatePrice(inv.id)}
                            >
                              Save
                            </button>
                          </div>
                        </Show>
                      </td>
                      <td class="py-2 px-2 text-right">{formatCurrency(inv.current_value)}</td>
                      <td class={`py-2 px-2 text-right ${gainClass(inv.gain_loss)}`}>
                        {formatCurrency(inv.gain_loss)}
                      </td>
                      <td class={`py-2 px-2 text-right ${gainClass(inv.gain_loss_pct)}`}>
                        {formatPct(inv.gain_loss_pct)}
                      </td>
                      <td class="py-2 px-2 text-center">
                        <div class="flex items-center justify-center gap-2">
                          <button
                            class="text-blue-600 hover:text-blue-800 dark:text-blue-400 text-xs underline"
                            onClick={() => {
                              setEditingPriceId(inv.id);
                              setEditPrice(String(inv.current_price));
                            }}
                          >
                            Update Price
                          </button>
                          <button
                            class="text-red-600 hover:text-red-800 dark:text-red-400 text-xs underline"
                            onClick={() => handleDeleteInvestment(inv.id)}
                          >
                            Delete
                          </button>
                        </div>
                      </td>
                    </tr>
                  )}
                </For>
              </tbody>
            </table>
          </div>
        </Show>
      </Show>

      {/* ==================== Transactions Tab ==================== */}
      <Show when={activeTab() === 'transactions'}>
        <h2 class="text-lg font-semibold mb-4">Recent Transactions</h2>
        <Show
          when={transactions().length > 0}
          fallback={
            <div class="card p-8 text-center text-gray-500 dark:text-gray-400">
              No transactions yet. Import a CSV or add transactions manually.
            </div>
          }
        >
          <div class="overflow-x-auto">
            <table class="w-full text-sm">
              <thead>
                <tr class="border-b border-gray-200 dark:border-gray-700 text-left text-gray-500 dark:text-gray-400">
                  <th class="py-2 px-2 font-medium">Date</th>
                  <th class="py-2 px-2 font-medium">Description</th>
                  <th class="py-2 px-2 font-medium">Category</th>
                  <th class="py-2 px-2 font-medium">Type</th>
                  <th class="py-2 px-2 font-medium text-right">Amount</th>
                </tr>
              </thead>
              <tbody>
                <For each={transactions()}>
                  {(tx) => (
                    <tr class="border-b border-gray-100 dark:border-gray-800">
                      <td class="py-2 px-2">{tx.date.split('T')[0]}</td>
                      <td class="py-2 px-2">{tx.description || '--'}</td>
                      <td class="py-2 px-2 text-gray-500">{tx.category || 'Uncategorized'}</td>
                      <td class="py-2 px-2">
                        <span
                          class={`inline-block px-2 py-0.5 rounded text-xs text-white ${
                            tx.transaction_type === 'credit' ? 'bg-green-500' : 'bg-red-500'
                          }`}
                        >
                          {tx.transaction_type}
                        </span>
                      </td>
                      <td
                        class={`py-2 px-2 text-right font-medium ${
                          tx.transaction_type === 'credit'
                            ? 'text-green-600 dark:text-green-400'
                            : 'text-red-600 dark:text-red-400'
                        }`}
                      >
                        {tx.transaction_type === 'credit' ? '+' : '-'}
                        {formatCurrency(tx.amount)}
                      </td>
                    </tr>
                  )}
                </For>
              </tbody>
            </table>
          </div>
        </Show>
      </Show>

      {/* ==================== Expenses Tab ==================== */}
      <Show when={activeTab() === 'expenses'}>
        {/* Month selector */}
        <div class="flex items-center justify-between mb-6">
          <h2 class="text-lg font-semibold">Expense Analytics</h2>
          <div class="flex items-center gap-3">
            <button
              class="btn btn-secondary text-sm px-3"
              onClick={() => {
                const m = prevMonth(expenseMonth());
                setExpenseMonth(m);
                loadExpenses(m);
              }}
            >
              &larr; Prev
            </button>
            <span class="text-sm font-medium min-w-[140px] text-center">
              {monthLabel(expenseMonth())}
            </span>
            <button
              class="btn btn-secondary text-sm px-3"
              disabled={expenseMonth() >= currentMonthStr()}
              onClick={() => {
                const m = nextMonth(expenseMonth());
                setExpenseMonth(m);
                loadExpenses(m);
              }}
            >
              Next &rarr;
            </button>
          </div>
        </div>

        {(() => {
          const cats = spendingByCategory();
          const entries = Object.entries(cats).sort(([, a], [, b]) => b - a);
          const total = entries.reduce((s, [, v]) => s + v, 0);
          const topTxns = monthTransactions().slice(0, 10);

          return (
            <Show
              when={entries.length > 0}
              fallback={
                <div class="card p-8 text-center text-gray-500 dark:text-gray-400">
                  No expenses found for {monthLabel(expenseMonth())}. Import transactions to see analytics.
                </div>
              }
            >
              <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
                {/* Category breakdown - horizontal bars */}
                <div class="card p-4">
                  <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-1">
                    Spending by Category
                  </h3>
                  <p class="text-xs text-gray-400 mb-4">
                    Total: {formatCurrency(total)}
                  </p>
                  <div class="space-y-3">
                    <For each={entries}>
                      {([cat, amount]) => {
                        const pct = total > 0 ? (amount / total) * 100 : 0;
                        const color = CATEGORY_COLORS[cat] || 'bg-gray-500';
                        return (
                          <div>
                            <div class="flex justify-between text-sm mb-1">
                              <span class="text-gray-700 dark:text-gray-300">{cat}</span>
                              <span class="text-gray-500">
                                {formatCurrency(amount)}{' '}
                                <span class="text-xs">({pct.toFixed(1)}%)</span>
                              </span>
                            </div>
                            <div class="w-full h-4 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
                              <div
                                class={`h-full rounded-full ${color} transition-all`}
                                style={{ width: `${Math.max(pct, 1)}%` }}
                              />
                            </div>
                          </div>
                        );
                      }}
                    </For>
                  </div>
                </div>

                {/* Top 10 expenses */}
                <div class="card p-4">
                  <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4">
                    Top 10 Expenses
                  </h3>
                  <Show
                    when={topTxns.length > 0}
                    fallback={
                      <p class="text-gray-400 text-sm">No debit transactions this month.</p>
                    }
                  >
                    <div class="space-y-2">
                      <For each={topTxns}>
                        {(tx, idx) => (
                          <div class="flex items-center gap-3 py-2 border-b border-gray-100 dark:border-gray-800 last:border-0">
                            <span class="text-xs text-gray-400 w-5 text-right">{idx() + 1}.</span>
                            <div class="flex-1 min-w-0">
                              <p class="text-sm font-medium truncate">
                                {tx.description || 'No description'}
                              </p>
                              <p class="text-xs text-gray-500">
                                {tx.date.split('T')[0]} {tx.category ? `| ${tx.category}` : ''}
                              </p>
                            </div>
                            <span class="text-sm font-medium text-red-600 dark:text-red-400 whitespace-nowrap">
                              {formatCurrency(tx.amount)}
                            </span>
                          </div>
                        )}
                      </For>
                    </div>
                  </Show>
                </div>
              </div>
            </Show>
          );
        })()}
      </Show>

      {/* ==================== Credit Cards Tab ==================== */}
      <Show when={activeTab() === 'creditcards'}>
        <div class="flex items-center justify-between mb-6">
          <h2 class="text-lg font-semibold">Credit Cards</h2>
          <button
            class="btn btn-primary"
            onClick={() => setShowCcForm(!showCcForm())}
          >
            {showCcForm() ? 'Cancel' : '+ Add Credit Card'}
          </button>
        </div>

        {/* Add credit card form */}
        <Show when={showCcForm()}>
          <form onSubmit={handleAddCreditCard} class="card p-4 mb-6">
            <h3 class="font-medium mb-3">Add Credit Card</h3>
            <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
              <div>
                <label class="block text-xs text-gray-500 mb-1">Card Name *</label>
                <input
                  type="text"
                  class="input w-full"
                  required
                  value={ccName()}
                  onInput={(e) => setCcName(e.currentTarget.value)}
                  placeholder="e.g. Amazon Pay ICICI"
                />
              </div>
              <div>
                <label class="block text-xs text-gray-500 mb-1">Bank *</label>
                <input
                  type="text"
                  class="input w-full"
                  required
                  value={ccBank()}
                  onInput={(e) => setCcBank(e.currentTarget.value)}
                  placeholder="e.g. ICICI Bank"
                />
              </div>
              <div>
                <label class="block text-xs text-gray-500 mb-1">Last 4 Digits</label>
                <input
                  type="text"
                  class="input w-full"
                  maxLength={4}
                  value={ccLast4()}
                  onInput={(e) => setCcLast4(e.currentTarget.value.replace(/\D/g, '').slice(0, 4))}
                  placeholder="1234"
                />
              </div>
              <div>
                <label class="block text-xs text-gray-500 mb-1">Credit Limit *</label>
                <input
                  type="number"
                  step="1"
                  class="input w-full"
                  required
                  value={ccLimit()}
                  onInput={(e) => setCcLimit(e.currentTarget.value)}
                  placeholder="e.g. 200000"
                />
              </div>
              <div>
                <label class="block text-xs text-gray-500 mb-1">Billing Date</label>
                <input
                  type="date"
                  class="input w-full"
                  value={ccBillingDate()}
                  onInput={(e) => setCcBillingDate(e.currentTarget.value)}
                />
              </div>
              <div>
                <label class="block text-xs text-gray-500 mb-1">Due Date</label>
                <input
                  type="date"
                  class="input w-full"
                  value={ccDueDate()}
                  onInput={(e) => setCcDueDate(e.currentTarget.value)}
                />
              </div>
            </div>
            <div class="mt-3 flex justify-end">
              <button type="submit" class="btn btn-primary">
                Add Credit Card
              </button>
            </div>
          </form>
        </Show>

        {/* Credit card list */}
        <Show
          when={creditCards().length > 0}
          fallback={
            <div class="card p-8 text-center text-gray-500 dark:text-gray-400">
              No credit cards added yet. Click "Add Credit Card" to get started.
            </div>
          }
        >
          {/* Summary bar */}
          <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
            <div class="card p-4">
              <p class="text-sm text-gray-500 dark:text-gray-400">Total Credit Limit</p>
              <p class="text-2xl font-bold">{formatCurrency(totalCreditLimit())}</p>
            </div>
            <div class="card p-4">
              <p class="text-sm text-gray-500 dark:text-gray-400">Total Outstanding</p>
              <p class="text-2xl font-bold text-red-600 dark:text-red-400">
                {formatCurrency(totalOutstanding())}
              </p>
            </div>
            <div class="card p-4">
              <p class="text-sm text-gray-500 dark:text-gray-400">Overall Utilization</p>
              <p class={`text-2xl font-bold ${ccUtilization() > 75 ? 'text-red-600 dark:text-red-400' : ccUtilization() > 30 ? 'text-yellow-600 dark:text-yellow-400' : 'text-green-600 dark:text-green-400'}`}>
                {ccUtilization().toFixed(1)}%
              </p>
            </div>
          </div>

          {/* Card grid */}
          <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            <For each={creditCards()}>
              {(cc) => {
                const meta = parseCcMeta(cc.institution);
                const outstanding = Math.abs(cc.balance);
                const limit = meta.credit_limit || 0;
                const available = Math.max(limit - outstanding, 0);
                const utilPct = limit > 0 ? (outstanding / limit) * 100 : 0;
                const days = meta.due_date ? daysUntil(meta.due_date) : 999;

                return (
                  <div class="card p-4 border-l-4" classList={{
                    'border-l-green-500': days > 7,
                    'border-l-yellow-500': days > 0 && days <= 7,
                    'border-l-red-500': days <= 0,
                  }}>
                    {/* Header */}
                    <div class="flex items-start justify-between mb-3">
                      <div>
                        <p class="font-semibold text-sm">{cc.name}</p>
                        <p class="text-xs text-gray-500">
                          {meta.bank}
                          {meta.last4 ? ` **** ${meta.last4}` : ''}
                        </p>
                      </div>
                      <span class="text-xs px-2 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-500">
                        Credit Card
                      </span>
                    </div>

                    {/* Usage bar */}
                    <div class="mb-3">
                      <div class="flex justify-between text-xs text-gray-500 mb-1">
                        <span>Used: {formatCurrency(outstanding)}</span>
                        <span>Limit: {formatCurrency(limit)}</span>
                      </div>
                      <div class="w-full h-3 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
                        <div
                          class={`h-full rounded-full transition-all ${
                            utilPct > 75
                              ? 'bg-red-500'
                              : utilPct > 30
                                ? 'bg-yellow-500'
                                : 'bg-green-500'
                          }`}
                          style={{ width: `${Math.min(utilPct, 100)}%` }}
                        />
                      </div>
                      <p class="text-xs text-gray-500 mt-1">
                        Available: {formatCurrency(available)} ({(100 - utilPct).toFixed(0)}%)
                      </p>
                    </div>

                    {/* Outstanding */}
                    <div class="flex justify-between items-center py-2 border-t border-gray-100 dark:border-gray-800">
                      <span class="text-xs text-gray-500">Outstanding</span>
                      <span class="text-sm font-bold text-red-600 dark:text-red-400">
                        {formatCurrency(outstanding)}
                      </span>
                    </div>

                    {/* Due date */}
                    <Show when={meta.due_date}>
                      <div class="flex justify-between items-center py-2 border-t border-gray-100 dark:border-gray-800">
                        <span class="text-xs text-gray-500">Due Date</span>
                        <span
                          class="text-xs font-medium"
                          classList={{
                            'text-green-600 dark:text-green-400': days > 7,
                            'text-yellow-600 dark:text-yellow-400': days > 0 && days <= 7,
                            'text-red-600 dark:text-red-400': days <= 0,
                          }}
                        >
                          {meta.due_date}
                          {days <= 0
                            ? ' (OVERDUE)'
                            : days <= 7
                              ? ` (${days}d left)`
                              : ` (${days}d)`}
                        </span>
                      </div>
                    </Show>

                    {/* Billing date */}
                    <Show when={meta.billing_date}>
                      <div class="flex justify-between items-center py-2 border-t border-gray-100 dark:border-gray-800">
                        <span class="text-xs text-gray-500">Bill Generated</span>
                        <span class="text-xs text-gray-500">{meta.billing_date}</span>
                      </div>
                    </Show>

                    {/* Min payment */}
                    <Show when={outstanding > 0}>
                      <div class="flex justify-between items-center py-2 border-t border-gray-100 dark:border-gray-800">
                        <span class="text-xs text-gray-500">Min. Payment (est.)</span>
                        <span class="text-xs font-medium text-orange-600 dark:text-orange-400">
                          {formatCurrency(Math.max(outstanding * 0.05, 200))}
                        </span>
                      </div>
                    </Show>
                  </div>
                );
              }}
            </For>
          </div>
        </Show>
      </Show>

      {/* ==================== Import Tab ==================== */}
      <Show when={activeTab() === 'import'}>
        <h2 class="text-lg font-semibold mb-4">Import Transactions from CSV</h2>
        <div class="card p-6 max-w-lg">
          <p class="text-sm text-gray-500 dark:text-gray-400 mb-4">
            Import bank/credit card statements in CSV format. The importer auto-detects common
            column layouts (Date, Description, Amount, Debit, Credit).
          </p>
          <div class="mb-4">
            <label class="block text-sm font-medium mb-1">Target Account</label>
            <Show
              when={accounts().length > 0}
              fallback={
                <p class="text-sm text-gray-500">
                  No accounts yet. Add an account first from the Overview tab.
                </p>
              }
            >
              <select
                class="input w-full"
                value={importAccountId()}
                onChange={(e) => setImportAccountId(e.currentTarget.value)}
              >
                <option value="">Select account...</option>
                <For each={accounts()}>
                  {(acct) => (
                    <option value={acct.id}>
                      {acct.name} ({acct.account_type}) - {formatCurrency(acct.balance)}
                    </option>
                  )}
                </For>
              </select>
            </Show>
          </div>
          <button
            class="btn btn-primary"
            disabled={!importAccountId()}
            onClick={handleImportCsv}
          >
            Choose CSV File & Import
          </button>
        </div>
      </Show>

      {/* ==================== CAGR Calculator Tab ==================== */}
      <Show when={activeTab() === 'cagr'}>
        <h2 class="text-lg font-semibold mb-4">CAGR Calculator</h2>
        <div class="card p-6 max-w-md">
          <p class="text-sm text-gray-500 dark:text-gray-400 mb-4">
            Calculate the Compound Annual Growth Rate for any investment.
          </p>
          <div class="space-y-3">
            <div>
              <label class="block text-sm font-medium mb-1">Initial Value</label>
              <input
                type="number"
                step="0.01"
                class="input w-full"
                placeholder="e.g. 100000"
                value={cagrInitial()}
                onInput={(e) => setCagrInitial(e.currentTarget.value)}
              />
            </div>
            <div>
              <label class="block text-sm font-medium mb-1">Current Value</label>
              <input
                type="number"
                step="0.01"
                class="input w-full"
                placeholder="e.g. 150000"
                value={cagrCurrent()}
                onInput={(e) => setCagrCurrent(e.currentTarget.value)}
              />
            </div>
            <div>
              <label class="block text-sm font-medium mb-1">Number of Years</label>
              <input
                type="number"
                step="0.1"
                class="input w-full"
                placeholder="e.g. 3"
                value={cagrYears()}
                onInput={(e) => setCagrYears(e.currentTarget.value)}
              />
            </div>
            <button
              class="btn btn-primary w-full"
              disabled={!cagrInitial() || !cagrCurrent() || !cagrYears()}
              onClick={handleCalcCagr}
            >
              Calculate CAGR
            </button>
            <Show when={cagrResult() !== null}>
              <div class="mt-4 p-4 bg-gray-50 dark:bg-gray-800 rounded-lg text-center">
                <p class="text-sm text-gray-500 dark:text-gray-400">
                  Compound Annual Growth Rate
                </p>
                <p class={`text-3xl font-bold ${gainClass(cagrResult()!)}`}>
                  {formatPct(cagrResult()!)}
                </p>
              </div>
            </Show>
          </div>
        </div>

        {/* MF NAV Lookup */}
        <h2 class="text-lg font-semibold mt-8 mb-4">Mutual Fund NAV Lookup</h2>
        <div class="card p-6 max-w-md">
          <p class="text-sm text-gray-500 dark:text-gray-400 mb-4">
            Fetch the latest Net Asset Value for any Indian mutual fund using its AMFI scheme code
            (e.g. 119551 for SBI Bluechip).
          </p>
          <div class="flex items-end gap-3">
            <div class="flex-1">
              <label class="block text-sm font-medium mb-1">Scheme Code</label>
              <input
                type="text"
                class="input w-full"
                placeholder="e.g. 119551"
                value={mfSchemeCode()}
                onInput={(e) => setMfSchemeCode(e.currentTarget.value)}
              />
            </div>
            <button
              class="btn btn-primary"
              disabled={mfLoading() || !mfSchemeCode()}
              onClick={handleFetchMfNav}
            >
              {mfLoading() ? 'Fetching...' : 'Fetch NAV'}
            </button>
          </div>
          <Show when={mfNav() !== null}>
            <div class="mt-4 p-4 bg-gray-50 dark:bg-gray-800 rounded-lg text-center">
              <p class="text-sm text-gray-500 dark:text-gray-400">Latest NAV</p>
              <p class="text-3xl font-bold text-green-600 dark:text-green-400">
                {formatCurrency(mfNav()!)}
              </p>
            </div>
          </Show>
        </div>
      </Show>

      {/* Loading overlay */}
      <Show when={loading()}>
        <div class="fixed inset-0 bg-black/10 flex items-center justify-center z-50 pointer-events-none">
          <div class="bg-white dark:bg-gray-800 rounded-lg p-4 shadow-lg pointer-events-auto">
            Loading...
          </div>
        </div>
      </Show>
    </div>
  );
};

export default Finance;
