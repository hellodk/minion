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

function formatCurrency(value: number): string {
  return new Intl.NumberFormat('en-IN', {
    style: 'currency',
    currency: 'INR',
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(value);
}

function formatPct(value: number): string {
  return `${value >= 0 ? '+' : ''}${value.toFixed(2)}%`;
}

function gainClass(value: number): string {
  if (value > 0) return 'text-green-600 dark:text-green-400';
  if (value < 0) return 'text-red-600 dark:text-red-400';
  return 'text-gray-500';
}

// ---------- Component ----------

const Finance: Component = () => {
  type TabId = 'overview' | 'investments' | 'transactions' | 'import' | 'cagr';
  const [activeTab, setActiveTab] = createSignal<TabId>('overview');

  // Data signals
  const [summary, setSummary] = createSignal<FinancialSummaryResponse | null>(null);
  const [portfolio, setPortfolio] = createSignal<PortfolioSummary | null>(null);
  const [investments, setInvestments] = createSignal<InvestmentResponse[]>([]);
  const [transactions, setTransactions] = createSignal<FinanceTransactionResponse[]>([]);
  const [accounts, setAccounts] = createSignal<FinanceAccountResponse[]>([]);

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

  // ---------- Data loading ----------

  async function loadAll() {
    setLoading(true);
    setError(null);
    try {
      const [sum, port, invs, txns, accts] = await Promise.all([
        invoke<FinancialSummaryResponse>('finance_get_summary'),
        invoke<PortfolioSummary>('finance_portfolio_summary'),
        invoke<InvestmentResponse[]>('finance_list_investments'),
        invoke<FinanceTransactionResponse[]>('finance_list_transactions', { limit: 100 }),
        invoke<FinanceAccountResponse[]>('finance_list_accounts'),
      ]);
      setSummary(sum);
      setPortfolio(port);
      setInvestments(invs);
      setTransactions(txns);
      setAccounts(accts);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  onMount(loadAll);

  // ---------- Portfolio CAGR computed ----------

  const portfolioCagr = createMemo(() => {
    const p = portfolio();
    if (!p || p.total_invested <= 0 || p.current_value <= 0) return null;
    // Estimate years from earliest investment
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
      // Reset form
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

  // ---------- Tabs ----------

  const tabs: { id: TabId; label: string }[] = [
    { id: 'overview', label: 'Overview' },
    { id: 'investments', label: 'Investments' },
    { id: 'transactions', label: 'Transactions' },
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
              onClick={() => setActiveTab(tab.id)}
            >
              {tab.label}
            </button>
          )}
        </For>
      </div>

      {/* ==================== Overview Tab ==================== */}
      <Show when={activeTab() === 'overview'}>
        {/* Financial summary cards */}
        <div class="grid grid-cols-1 md:grid-cols-4 gap-4 mb-8">
          <div class="card p-4">
            <p class="text-sm text-gray-500 dark:text-gray-400">Net Worth</p>
            <p class="text-2xl font-bold">{summary() ? formatCurrency(summary()!.net_worth) : '--'}</p>
          </div>
          <div class="card p-4">
            <p class="text-sm text-gray-500 dark:text-gray-400">Monthly Income</p>
            <p class="text-2xl font-bold">
              {summary() ? formatCurrency(summary()!.monthly_income) : '--'}
            </p>
          </div>
          <div class="card p-4">
            <p class="text-sm text-gray-500 dark:text-gray-400">Monthly Expenses</p>
            <p class="text-2xl font-bold">
              {summary() ? formatCurrency(summary()!.monthly_expenses) : '--'}
            </p>
          </div>
          <div class="card p-4">
            <p class="text-sm text-gray-500 dark:text-gray-400">Savings Rate</p>
            <p class="text-2xl font-bold">
              {summary() ? `${summary()!.savings_rate.toFixed(1)}%` : '--%'}
            </p>
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
