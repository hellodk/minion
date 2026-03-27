import { Component, createSignal } from 'solid-js';

const Finance: Component = () => {
  const [activeTab, setActiveTab] = createSignal<'overview' | 'transactions' | 'investments' | 'goals'>('overview');

  return (
    <div class="p-6">
      <h1 class="text-2xl font-bold mb-6">Finance Intelligence</h1>

      {/* Summary Cards */}
      <div class="grid grid-cols-1 md:grid-cols-4 gap-4 mb-8">
        <div class="card p-4">
          <p class="text-sm text-gray-500 dark:text-gray-400">Net Worth</p>
          <p class="text-2xl font-bold">₹ --</p>
          <p class="text-xs text-green-600">-</p>
        </div>
        <div class="card p-4">
          <p class="text-sm text-gray-500 dark:text-gray-400">Monthly Income</p>
          <p class="text-2xl font-bold">₹ --</p>
        </div>
        <div class="card p-4">
          <p class="text-sm text-gray-500 dark:text-gray-400">Monthly Expenses</p>
          <p class="text-2xl font-bold">₹ --</p>
        </div>
        <div class="card p-4">
          <p class="text-sm text-gray-500 dark:text-gray-400">Savings Rate</p>
          <p class="text-2xl font-bold">--%</p>
        </div>
      </div>

      {/* Tabs */}
      <div class="flex border-b border-gray-200 dark:border-gray-700 mb-6">
        <button
          class="px-4 py-2 -mb-px border-b-2 transition-colors"
          classList={{
            'border-minion-500 text-minion-600 dark:text-minion-400': activeTab() === 'overview',
            'border-transparent text-gray-500': activeTab() !== 'overview',
          }}
          onClick={() => setActiveTab('overview')}
        >
          Overview
        </button>
        <button
          class="px-4 py-2 -mb-px border-b-2 transition-colors"
          classList={{
            'border-minion-500 text-minion-600 dark:text-minion-400': activeTab() === 'transactions',
            'border-transparent text-gray-500': activeTab() !== 'transactions',
          }}
          onClick={() => setActiveTab('transactions')}
        >
          Transactions
        </button>
        <button
          class="px-4 py-2 -mb-px border-b-2 transition-colors"
          classList={{
            'border-minion-500 text-minion-600 dark:text-minion-400': activeTab() === 'investments',
            'border-transparent text-gray-500': activeTab() !== 'investments',
          }}
          onClick={() => setActiveTab('investments')}
        >
          Investments
        </button>
        <button
          class="px-4 py-2 -mb-px border-b-2 transition-colors"
          classList={{
            'border-minion-500 text-minion-600 dark:text-minion-400': activeTab() === 'goals',
            'border-transparent text-gray-500': activeTab() !== 'goals',
          }}
          onClick={() => setActiveTab('goals')}
        >
          Goals
        </button>
      </div>

      {/* Content */}
      <div class="card p-8 text-center">
        <svg class="w-16 h-16 mx-auto mb-4 text-gray-300 dark:text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
        </svg>
        <h3 class="text-lg font-medium mb-2">Set up your finances</h3>
        <p class="text-gray-500 dark:text-gray-400 mb-4">
          Add your bank accounts and start tracking expenses
        </p>
        <button class="btn btn-primary">
          Add Account
        </button>
      </div>
    </div>
  );
};

export default Finance;
