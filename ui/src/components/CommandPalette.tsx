import {
  Component,
  createSignal,
  createEffect,
  onCleanup,
  For,
  Show,
  createMemo,
} from 'solid-js';

export interface Command {
  id: string;
  label: string;
  description: string;
  shortcut?: string;
  icon?: string;
  action: () => void;
}

interface CommandPaletteProps {
  navigate: (path: string) => void;
  toggleDarkMode: () => void;
}

const CommandPalette: Component<CommandPaletteProps> = (props) => {
  const [open, setOpen] = createSignal(false);
  const [query, setQuery] = createSignal('');
  const [activeIndex, setActiveIndex] = createSignal(0);

  let inputRef: HTMLInputElement | undefined;

  const commands = createMemo<Command[]>(() => [
    {
      id: 'go-dashboard',
      label: 'Go to Dashboard',
      description: 'Navigate to the main dashboard',
      shortcut: '',
      icon: '\u{1F3E0}',
      action: () => props.navigate('/'),
    },
    {
      id: 'go-files',
      label: 'Go to Files',
      description: 'Browse and manage your files',
      shortcut: '',
      icon: '\u{1F4C1}',
      action: () => props.navigate('/files'),
    },
    {
      id: 'go-reader',
      label: 'Go to Reader',
      description: 'Open the reading library',
      shortcut: '',
      icon: '\u{1F4D6}',
      action: () => props.navigate('/reader'),
    },
    {
      id: 'go-finance',
      label: 'Go to Finance',
      description: 'View financial overview',
      shortcut: '',
      icon: '\u{1F4B0}',
      action: () => props.navigate('/finance'),
    },
    {
      id: 'go-fitness',
      label: 'Go to Fitness',
      description: 'Track workouts and health',
      shortcut: '',
      icon: '\u{1F4AA}',
      action: () => props.navigate('/fitness'),
    },
    {
      id: 'go-settings',
      label: 'Go to Settings',
      description: 'Configure application settings',
      shortcut: '',
      icon: '\u2699\uFE0F',
      action: () => props.navigate('/settings'),
    },
    {
      id: 'scan-directory',
      label: 'Scan Directory',
      description: 'Navigate to Files and scan a directory',
      shortcut: '',
      icon: '\u{1F50D}',
      action: () => props.navigate('/files'),
    },
    {
      id: 'open-book',
      label: 'Open Book',
      description: 'Navigate to Reader to open a book',
      shortcut: '',
      icon: '\u{1F4DA}',
      action: () => props.navigate('/reader'),
    },
    {
      id: 'toggle-dark-mode',
      label: 'Toggle Dark Mode',
      description: 'Switch to dark theme',
      shortcut: '',
      icon: '\u{1F319}',
      action: () => props.toggleDarkMode(),
    },
    {
      id: 'toggle-light-mode',
      label: 'Toggle Light Mode',
      description: 'Switch to light theme',
      shortcut: '',
      icon: '\u2600\uFE0F',
      action: () => props.toggleDarkMode(),
    },
  ]);

  const filtered = createMemo(() => {
    const q = query().toLowerCase().trim();
    if (!q) return commands();
    return commands().filter(
      (cmd) =>
        cmd.label.toLowerCase().includes(q) ||
        cmd.description.toLowerCase().includes(q)
    );
  });

  // Reset active index when filtered results change
  createEffect(() => {
    filtered();
    setActiveIndex(0);
  });

  // Focus input when palette opens
  createEffect(() => {
    if (open()) {
      // Small delay to allow DOM to render
      requestAnimationFrame(() => {
        inputRef?.focus();
      });
    } else {
      setQuery('');
      setActiveIndex(0);
    }
  });

  // Global keyboard listener for Ctrl+K / Cmd+K
  const handleGlobalKeydown = (e: KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
      e.preventDefault();
      setOpen((prev) => !prev);
    }
  };

  if (typeof window !== 'undefined') {
    window.addEventListener('keydown', handleGlobalKeydown);
    onCleanup(() => {
      window.removeEventListener('keydown', handleGlobalKeydown);
    });
  }

  const executeCommand = (cmd: Command) => {
    setOpen(false);
    cmd.action();
  };

  const handleKeydown = (e: KeyboardEvent) => {
    const items = filtered();
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        setActiveIndex((i) => (i + 1) % items.length);
        break;
      case 'ArrowUp':
        e.preventDefault();
        setActiveIndex((i) => (i - 1 + items.length) % items.length);
        break;
      case 'Enter':
        e.preventDefault();
        if (items[activeIndex()]) {
          executeCommand(items[activeIndex()]);
        }
        break;
      case 'Escape':
        e.preventDefault();
        setOpen(false);
        break;
    }
  };

  // Scroll active item into view
  createEffect(() => {
    const idx = activeIndex();
    const el = document.querySelector(
      `[data-command-index="${idx}"]`
    );
    el?.scrollIntoView({ block: 'nearest' });
  });

  return (
    <Show when={open()}>
      {/* Backdrop */}
      <div
        class="fixed inset-0 z-50 flex items-start justify-center pt-[20vh]"
        onClick={() => setOpen(false)}
      >
        <div
          class="fixed inset-0 bg-black/50 backdrop-blur-sm
                 animate-[fadeIn_150ms_ease-out]"
        />

        {/* Card */}
        <div
          class="relative z-10 w-full max-w-lg mx-4
                 bg-white dark:bg-gray-800
                 rounded-xl shadow-2xl
                 border border-gray-200 dark:border-gray-700
                 overflow-hidden
                 animate-[scaleIn_150ms_ease-out]"
          onClick={(e) => e.stopPropagation()}
          onKeyDown={handleKeydown}
        >
          {/* Search input */}
          <div class="flex items-center px-4 border-b border-gray-200 dark:border-gray-700">
            <svg
              class="w-5 h-5 text-gray-400 shrink-0"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
              />
            </svg>
            <input
              ref={inputRef}
              type="text"
              placeholder="Type a command..."
              class="w-full py-4 px-3 text-lg bg-transparent
                     text-gray-900 dark:text-gray-100
                     placeholder-gray-400 dark:placeholder-gray-500
                     outline-none border-none"
              value={query()}
              onInput={(e) => setQuery(e.currentTarget.value)}
            />
            <kbd
              class="hidden sm:inline-flex items-center px-2 py-0.5
                     text-xs font-medium text-gray-400 dark:text-gray-500
                     bg-gray-100 dark:bg-gray-700 rounded
                     border border-gray-200 dark:border-gray-600"
            >
              ESC
            </kbd>
          </div>

          {/* Results */}
          <div class="max-h-72 overflow-y-auto py-2">
            <Show
              when={filtered().length > 0}
              fallback={
                <div class="px-4 py-8 text-center text-gray-400 dark:text-gray-500">
                  No commands found
                </div>
              }
            >
              <For each={filtered()}>
                {(cmd, index) => (
                  <button
                    data-command-index={index()}
                    class="w-full flex items-center gap-3 px-4 py-2.5
                           text-left transition-colors duration-75 cursor-pointer"
                    classList={{
                      'bg-minion-50 dark:bg-minion-900/50':
                        activeIndex() === index(),
                      'hover:bg-gray-50 dark:hover:bg-gray-700/50':
                        activeIndex() !== index(),
                    }}
                    onClick={() => executeCommand(cmd)}
                    onMouseEnter={() => setActiveIndex(index())}
                  >
                    <span class="text-lg w-7 text-center shrink-0">
                      {cmd.icon}
                    </span>
                    <div class="flex-1 min-w-0">
                      <div class="text-sm font-medium text-gray-900 dark:text-gray-100">
                        {cmd.label}
                      </div>
                      <div class="text-xs text-gray-500 dark:text-gray-400 truncate">
                        {cmd.description}
                      </div>
                    </div>
                    <Show when={cmd.shortcut}>
                      <kbd
                        class="hidden sm:inline-flex items-center px-1.5 py-0.5
                               text-[10px] font-medium text-gray-400 dark:text-gray-500
                               bg-gray-100 dark:bg-gray-700 rounded
                               border border-gray-200 dark:border-gray-600"
                      >
                        {cmd.shortcut}
                      </kbd>
                    </Show>
                  </button>
                )}
              </For>
            </Show>
          </div>

          {/* Footer hint */}
          <div
            class="flex items-center justify-between px-4 py-2
                   border-t border-gray-200 dark:border-gray-700
                   text-xs text-gray-400 dark:text-gray-500"
          >
            <div class="flex items-center gap-2">
              <span class="flex items-center gap-1">
                <kbd class="px-1 py-0.5 bg-gray-100 dark:bg-gray-700 rounded text-[10px]">
                  &uarr;&darr;
                </kbd>
                navigate
              </span>
              <span class="flex items-center gap-1">
                <kbd class="px-1 py-0.5 bg-gray-100 dark:bg-gray-700 rounded text-[10px]">
                  &crarr;
                </kbd>
                select
              </span>
              <span class="flex items-center gap-1">
                <kbd class="px-1 py-0.5 bg-gray-100 dark:bg-gray-700 rounded text-[10px]">
                  esc
                </kbd>
                close
              </span>
            </div>
          </div>
        </div>
      </div>
    </Show>
  );
};

export default CommandPalette;
