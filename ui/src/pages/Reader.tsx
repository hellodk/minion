import { Component, createSignal, onMount, onCleanup, For, Show } from 'solid-js';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

interface Book {
  id: string;
  title: string;
  authors: string[];
  path: string;
  format: string;
  cover_url?: string;
}

interface ChapterInfo {
  index: number;
  title: string;
  content: string;
}

interface TocEntry {
  title: string;
  href: string;
}

interface BookContent {
  metadata: {
    title: string;
    authors: string[];
    publisher?: string;
    language?: string;
    description?: string;
  };
  chapters: ChapterInfo[];
  toc: TocEntry[];
  file_path?: string;
  format: string;
}

type ReadingMode = 'light' | 'dark' | 'sepia';

const Reader: Component = () => {
  const [books, setBooks] = createSignal<Book[]>([]);
  const [currentBook, setCurrentBook] = createSignal<BookContent | null>(null);
  const [currentChapter, setCurrentChapter] = createSignal(0);
  const [view, setView] = createSignal<'library' | 'reader'>('library');
  const [bookPath, setBookPath] = createSignal('');
  const [libraryPath, setLibraryPath] = createSignal('');
  const [loading, setLoading] = createSignal(false);
  const [showToc, setShowToc] = createSignal(false);
  const [fontSize, setFontSize] = createSignal(16);
  const [readingMode, setReadingMode] = createSignal<ReadingMode>('light');

  // Animation signals
  const [pageDirection, setPageDirection] = createSignal<'left' | 'right'>('left');
  const [pageTransitioning, setPageTransitioning] = createSignal(false);
  const [bookOpening, setBookOpening] = createSignal(false);
  const [bookClosing, setBookClosing] = createSignal(false);
  const [openingCardIndex, setOpeningCardIndex] = createSignal<number | null>(null);

  // Hover tilt state
  const [hoveredCard, setHoveredCard] = createSignal<string | null>(null);
  const [tiltX, setTiltX] = createSignal(0);
  const [tiltY, setTiltY] = createSignal(0);

  // Keyboard navigation
  const handleKeyDown = (e: KeyboardEvent) => {
    if (view() !== 'reader' || !currentBook()) return;
    if (pageTransitioning()) return;

    if (e.key === 'ArrowRight') {
      e.preventDefault();
      nextChapter();
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault();
      prevChapter();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      closeBook();
    }
  };

  onMount(() => {
    document.addEventListener('keydown', handleKeyDown);
  });

  onCleanup(() => {
    document.removeEventListener('keydown', handleKeyDown);
  });

  const loadBooksFromDirectory = async () => {
    const path = libraryPath().trim();
    if (!path) return;

    setLoading(true);
    try {
      const bookList = await invoke<Book[]>('reader_list_books', { directory: path });
      setBooks(bookList);
    } catch (e) {
      console.error('Failed to load books:', e);
      alert(`Error: ${e}`);
    } finally {
      setLoading(false);
    }
  };

  const browseForBook = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: 'Books', extensions: ['epub', 'pdf', 'txt', 'md', 'markdown'] }]
      });
      if (selected && typeof selected === 'string') {
        setBookPath(selected);
        await openBook(selected);
      }
    } catch (e) {
      console.error('Failed to open file dialog:', e);
      alert(`Error opening file dialog: ${e}`);
    }
  };

  const browseForLibrary = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false
      });
      if (selected && typeof selected === 'string') {
        setLibraryPath(selected);
        await loadBooksFromPath(selected);
      }
    } catch (e) {
      console.error('Failed to open folder dialog:', e);
      alert(`Error opening folder dialog: ${e}`);
    }
  };

  const loadBooksFromPath = async (path: string) => {
    setLoading(true);
    try {
      const bookList = await invoke<Book[]>('reader_list_books', { directory: path });
      setBooks(bookList);
    } catch (e) {
      console.error('Failed to load books:', e);
      alert(`Error: ${e}`);
    } finally {
      setLoading(false);
    }
  };

  const openBook = async (path: string, cardIndex?: number) => {
    setLoading(true);
    try {
      const content = await invoke<BookContent>('reader_open_book', { path });
      setCurrentBook(content);
      setCurrentChapter(0);

      // Trigger open animation
      if (cardIndex !== undefined) {
        setOpeningCardIndex(cardIndex);
      }
      setBookOpening(true);
      setView('reader');

      // Clear animation state after it completes
      setTimeout(() => {
        setBookOpening(false);
        setOpeningCardIndex(null);
      }, 500);
    } catch (e) {
      console.error('Failed to open book:', e);
      alert(`Error: ${e}`);
    } finally {
      setLoading(false);
    }
  };

  const openBookDirect = async () => {
    const path = bookPath().trim();
    if (!path) return;
    await openBook(path);
  };

  const closeBook = () => {
    if (bookClosing()) return;
    setBookClosing(true);
    setTimeout(() => {
      setCurrentBook(null);
      setView('library');
      setBookClosing(false);
      setShowToc(false);
    }, 350);
  };

  const nextChapter = () => {
    const book = currentBook();
    if (!book || currentChapter() >= book.chapters.length - 1) return;
    if (pageTransitioning()) return;

    setPageDirection('left');
    setPageTransitioning(true);

    setTimeout(() => {
      setCurrentChapter(currentChapter() + 1);
      // Scroll reading area back to top
      const contentEl = document.getElementById('reader-content-scroll');
      if (contentEl) contentEl.scrollTop = 0;
    }, 300);

    setTimeout(() => {
      setPageTransitioning(false);
    }, 600);
  };

  const prevChapter = () => {
    if (currentChapter() <= 0) return;
    if (pageTransitioning()) return;

    setPageDirection('right');
    setPageTransitioning(true);

    setTimeout(() => {
      setCurrentChapter(currentChapter() - 1);
      const contentEl = document.getElementById('reader-content-scroll');
      if (contentEl) contentEl.scrollTop = 0;
    }, 300);

    setTimeout(() => {
      setPageTransitioning(false);
    }, 600);
  };

  const goToChapter = (index: number) => {
    if (pageTransitioning()) return;
    const dir = index > currentChapter() ? 'left' : 'right';
    setPageDirection(dir);
    setPageTransitioning(true);

    setTimeout(() => {
      setCurrentChapter(index);
      setShowToc(false);
      const contentEl = document.getElementById('reader-content-scroll');
      if (contentEl) contentEl.scrollTop = 0;
    }, 300);

    setTimeout(() => {
      setPageTransitioning(false);
    }, 600);
  };

  const handleCardMouseMove = (e: MouseEvent, bookId: string) => {
    const card = e.currentTarget as HTMLElement;
    const rect = card.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const centerX = rect.width / 2;
    const centerY = rect.height / 2;

    // Calculate tilt (-12 to 12 degrees)
    const rotateX = ((y - centerY) / centerY) * -12;
    const rotateY = ((x - centerX) / centerX) * 12;

    setHoveredCard(bookId);
    setTiltX(rotateX);
    setTiltY(rotateY);
  };

  const handleCardMouseLeave = () => {
    setHoveredCard(null);
    setTiltX(0);
    setTiltY(0);
  };

  const progressPercent = () => {
    const book = currentBook();
    if (!book || book.chapters.length === 0) return 0;
    return ((currentChapter() + 1) / book.chapters.length) * 100;
  };

  const modeStyles = () => {
    switch (readingMode()) {
      case 'sepia':
        return {
          bg: '#f4ecd8',
          text: '#5b4636',
          headerBg: '#ede0c8',
          headerBorder: '#d4c5a9',
          sidebarBg: '#eee2cc',
          contentBg: '#f4ecd8',
          mutedText: '#8b7355',
          activeBg: '#ddd0b4',
          hoverBg: '#e8dbc2',
          progressBar: '#a08060',
          navBg: '#ede0c8',
          navBorder: '#d4c5a9',
          chapterTitle: '#3e2c1c',
          prose: 'sepia-prose',
        };
      case 'dark':
        return {
          bg: '#1a1a2e',
          text: '#d4d4e4',
          headerBg: '#16213e',
          headerBorder: '#2a2a4a',
          sidebarBg: '#141428',
          contentBg: '#1a1a2e',
          mutedText: '#8888aa',
          activeBg: '#2a2a5a',
          hoverBg: '#222244',
          progressBar: '#6366f1',
          navBg: '#16213e',
          navBorder: '#2a2a4a',
          chapterTitle: '#e4e4f4',
          prose: 'dark-prose',
        };
      default: // light
        return {
          bg: '#ffffff',
          text: '#1f2937',
          headerBg: '#ffffff',
          headerBorder: '#e5e7eb',
          sidebarBg: '#f9fafb',
          contentBg: '#ffffff',
          mutedText: '#6b7280',
          activeBg: '#e0f2fe',
          hoverBg: '#f3f4f6',
          progressBar: '#0ea5e9',
          navBg: '#ffffff',
          navBorder: '#e5e7eb',
          chapterTitle: '#111827',
          prose: 'light-prose',
        };
    }
  };

  return (
    <>
      <style>{`
        /* Book card 3D tilt effect */
        .book-card-3d {
          perspective: 800px;
          transform-style: preserve-3d;
        }

        .book-card-inner {
          transition: transform 0.15s ease-out, box-shadow 0.3s ease;
          transform-style: preserve-3d;
          will-change: transform;
        }

        .book-card-inner:hover {
          box-shadow:
            0 20px 40px rgba(0, 0, 0, 0.15),
            0 8px 16px rgba(0, 0, 0, 0.1);
        }

        .book-card-3d .book-cover-shine {
          position: absolute;
          inset: 0;
          border-radius: 0.5rem;
          background: linear-gradient(
            105deg,
            transparent 40%,
            rgba(255, 255, 255, 0.12) 45%,
            rgba(255, 255, 255, 0.18) 50%,
            transparent 55%
          );
          pointer-events: none;
          opacity: 0;
          transition: opacity 0.3s ease;
        }

        .book-card-inner:hover .book-cover-shine {
          opacity: 1;
        }

        /* Book spine effect */
        .book-card-inner::before {
          content: '';
          position: absolute;
          left: 0;
          top: 4%;
          bottom: 4%;
          width: 3px;
          background: linear-gradient(
            to bottom,
            rgba(0,0,0,0.08),
            rgba(0,0,0,0.2),
            rgba(0,0,0,0.08)
          );
          border-radius: 1px;
          z-index: 2;
          transform: translateZ(1px);
        }

        /* Page turn animations */
        @keyframes pageExitLeft {
          0% {
            transform: translateX(0) scale(1);
            opacity: 1;
          }
          100% {
            transform: translateX(-60px) scale(0.97);
            opacity: 0;
          }
        }

        @keyframes pageEnterRight {
          0% {
            transform: translateX(60px) scale(0.97);
            opacity: 0;
          }
          100% {
            transform: translateX(0) scale(1);
            opacity: 1;
          }
        }

        @keyframes pageExitRight {
          0% {
            transform: translateX(0) scale(1);
            opacity: 1;
          }
          100% {
            transform: translateX(60px) scale(0.97);
            opacity: 0;
          }
        }

        @keyframes pageEnterLeft {
          0% {
            transform: translateX(-60px) scale(0.97);
            opacity: 0;
          }
          100% {
            transform: translateX(0) scale(1);
            opacity: 1;
          }
        }

        .page-exit-left {
          animation: pageExitLeft 0.3s ease-in forwards;
        }

        .page-enter-right {
          animation: pageEnterRight 0.3s ease-out forwards;
        }

        .page-exit-right {
          animation: pageExitRight 0.3s ease-in forwards;
        }

        .page-enter-left {
          animation: pageEnterLeft 0.3s ease-out forwards;
        }

        /* Book open animation */
        @keyframes bookOpen {
          0% {
            transform: scale(0.6);
            opacity: 0;
            filter: blur(4px);
          }
          60% {
            transform: scale(1.02);
            opacity: 0.9;
            filter: blur(0);
          }
          100% {
            transform: scale(1);
            opacity: 1;
            filter: blur(0);
          }
        }

        @keyframes bookClose {
          0% {
            transform: scale(1);
            opacity: 1;
            filter: blur(0);
          }
          100% {
            transform: scale(0.7);
            opacity: 0;
            filter: blur(4px);
          }
        }

        .book-opening {
          animation: bookOpen 0.5s cubic-bezier(0.16, 1, 0.3, 1) forwards;
        }

        .book-closing {
          animation: bookClose 0.35s ease-in forwards;
        }

        /* Reading progress bar */
        .reading-progress-bar {
          height: 3px;
          transition: width 0.4s cubic-bezier(0.4, 0, 0.2, 1);
        }

        /* Sepia mode prose styles */
        .sepia-prose {
          color: #5b4636;
        }

        .sepia-prose h1, .sepia-prose h2, .sepia-prose h3,
        .sepia-prose h4, .sepia-prose h5, .sepia-prose h6 {
          color: #3e2c1c;
        }

        .sepia-prose a {
          color: #8b5e3c;
        }

        .sepia-prose strong {
          color: #3e2c1c;
        }

        .sepia-prose blockquote {
          border-left-color: #c4a882;
          color: #7a6048;
        }

        .sepia-prose code {
          color: #6b4c36;
          background: rgba(0, 0, 0, 0.06);
        }

        .sepia-prose hr {
          border-color: #d4c5a9;
        }

        /* Dark mode prose */
        .dark-prose {
          color: #d4d4e4;
        }

        .dark-prose h1, .dark-prose h2, .dark-prose h3,
        .dark-prose h4, .dark-prose h5, .dark-prose h6 {
          color: #e4e4f4;
        }

        .dark-prose a {
          color: #818cf8;
        }

        .dark-prose strong {
          color: #e4e4f4;
        }

        .dark-prose blockquote {
          border-left-color: #4a4a6a;
          color: #a0a0c0;
        }

        .dark-prose code {
          color: #c4b5fd;
          background: rgba(255, 255, 255, 0.08);
        }

        .dark-prose hr {
          border-color: #2a2a4a;
        }

        /* Light prose */
        .light-prose {
          color: #374151;
        }

        .light-prose h1, .light-prose h2, .light-prose h3,
        .light-prose h4, .light-prose h5, .light-prose h6 {
          color: #111827;
        }

        .light-prose a {
          color: #0369a1;
        }

        .light-prose strong {
          color: #111827;
        }

        .light-prose blockquote {
          border-left-color: #d1d5db;
          color: #6b7280;
        }

        .light-prose code {
          color: #0369a1;
          background: rgba(0, 0, 0, 0.05);
        }

        /* Shared prose improvements */
        .sepia-prose, .dark-prose, .light-prose {
          line-height: 1.85;
          letter-spacing: 0.01em;
        }

        .sepia-prose p, .dark-prose p, .light-prose p {
          margin-bottom: 1.25em;
        }

        .sepia-prose h1, .dark-prose h1, .light-prose h1 {
          font-size: 1.75em;
          margin-top: 1.5em;
          margin-bottom: 0.6em;
          font-weight: 700;
          letter-spacing: -0.02em;
        }

        .sepia-prose h2, .dark-prose h2, .light-prose h2 {
          font-size: 1.4em;
          margin-top: 1.4em;
          margin-bottom: 0.5em;
          font-weight: 600;
        }

        .sepia-prose blockquote, .dark-prose blockquote, .light-prose blockquote {
          padding-left: 1.2em;
          border-left-width: 3px;
          font-style: italic;
          margin: 1.5em 0;
        }

        /* Reading mode toggle button */
        .mode-btn {
          width: 28px;
          height: 28px;
          border-radius: 50%;
          border: 2px solid transparent;
          cursor: pointer;
          transition: border-color 0.2s, transform 0.15s;
          display: flex;
          align-items: center;
          justify-content: center;
        }

        .mode-btn:hover {
          transform: scale(1.12);
        }

        .mode-btn.active {
          border-color: #0ea5e9;
          box-shadow: 0 0 0 2px rgba(14, 165, 233, 0.25);
        }

        .mode-btn-light {
          background: #ffffff;
          border-color: #d1d5db;
        }

        .mode-btn-light.active {
          border-color: #0ea5e9;
        }

        .mode-btn-sepia {
          background: #f4ecd8;
          border-color: #d4c5a9;
        }

        .mode-btn-sepia.active {
          border-color: #0ea5e9;
        }

        .mode-btn-dark {
          background: #1a1a2e;
          border-color: #3a3a5a;
        }

        .mode-btn-dark.active {
          border-color: #0ea5e9;
        }

        /* TOC sidebar slide animation */
        .toc-sidebar {
          transition: width 0.25s ease, opacity 0.2s ease;
          overflow: hidden;
        }

        /* Smooth scroll for reader content */
        #reader-content-scroll {
          scroll-behavior: smooth;
        }

        /* Library card opening pulse */
        @keyframes cardPulse {
          0% { transform: scale(1); }
          50% { transform: scale(0.96); }
          100% { transform: scale(1); }
        }

        .card-opening {
          animation: cardPulse 0.3s ease;
        }

        /* Navigation buttons */
        .nav-btn {
          transition: all 0.2s ease;
        }

        .nav-btn:not(:disabled):hover {
          transform: translateX(var(--nav-hover-x, 0));
        }

        .nav-btn:disabled {
          opacity: 0.35;
          cursor: not-allowed;
        }
      `}</style>

      <div class="h-full flex flex-col">
        <Show when={view() === 'library'}>
          <div class="p-6 flex-1 overflow-auto">
            <div class="flex items-center justify-between mb-6">
              <h1 class="text-2xl font-bold">Book Reader</h1>
            </div>

            {/* Open single book */}
            <div class="card p-4 mb-6">
              <h3 class="font-medium mb-3">Open a Book</h3>
              <div class="flex gap-2">
                <input
                  type="text"
                  class="input flex-1"
                  placeholder="Enter path to book file (e.g., /path/to/book.epub)"
                  value={bookPath()}
                  onInput={(e) => setBookPath(e.currentTarget.value)}
                  onKeyPress={(e) => e.key === 'Enter' && openBookDirect()}
                />
                <button
                  class="btn btn-secondary"
                  onClick={browseForBook}
                  disabled={loading()}
                  title="Browse for a book file"
                >
                  <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                  </svg>
                </button>
                <button
                  class="btn btn-primary"
                  onClick={openBookDirect}
                  disabled={loading() || !bookPath().trim()}
                >
                  {loading() ? 'Opening...' : 'Open'}
                </button>
              </div>
              <p class="text-xs text-gray-500 mt-2">
                Supported formats: EPUB, PDF, TXT, Markdown
              </p>
            </div>

            {/* Browse library */}
            <div class="card p-4 mb-6">
              <h3 class="font-medium mb-3">Browse Library Folder</h3>
              <div class="flex gap-2">
                <input
                  type="text"
                  class="input flex-1"
                  placeholder="Enter path to folder containing books"
                  value={libraryPath()}
                  onInput={(e) => setLibraryPath(e.currentTarget.value)}
                  onKeyPress={(e) => e.key === 'Enter' && loadBooksFromDirectory()}
                />
                <button
                  class="btn btn-secondary"
                  onClick={browseForLibrary}
                  disabled={loading()}
                  title="Browse for a folder"
                >
                  <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                  </svg>
                </button>
                <button
                  class="btn btn-primary"
                  onClick={loadBooksFromDirectory}
                  disabled={loading() || !libraryPath().trim()}
                >
                  {loading() ? 'Loading...' : 'Load'}
                </button>
              </div>
            </div>

            {/* Book grid with 3D hover cards */}
            <Show when={books().length > 0}>
              <h3 class="font-medium mb-4">Books Found ({books().length})</h3>
              <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-5">
                <For each={books()}>
                  {(book, index) => (
                    <div
                      class="book-card-3d cursor-pointer"
                      classList={{
                        'card-opening': openingCardIndex() === index(),
                      }}
                      onClick={() => openBook(book.path, index())}
                      onMouseMove={(e) => handleCardMouseMove(e, book.id)}
                      onMouseLeave={handleCardMouseLeave}
                    >
                      <div
                        class="book-card-inner card p-3 relative overflow-hidden"
                        style={{
                          transform: hoveredCard() === book.id
                            ? `rotateX(${tiltX()}deg) rotateY(${tiltY()}deg) translateZ(8px)`
                            : 'rotateX(0deg) rotateY(0deg) translateZ(0px)',
                        }}
                      >
                        <div class="book-cover-shine" />
                        <div class="aspect-[2/3] bg-gradient-to-br from-minion-100 to-minion-200 dark:from-minion-900 dark:to-minion-800 rounded-lg mb-2 flex items-center justify-center relative overflow-hidden">
                          <Show
                            when={book.cover_url}
                            fallback={
                              <svg class="w-12 h-12 text-minion-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
                              </svg>
                            }
                          >
                            <img
                              src={book.cover_url}
                              alt={book.title}
                              class="w-full h-full object-cover rounded-lg"
                            />
                          </Show>
                        </div>
                        <p class="font-medium text-sm truncate" title={book.title}>{book.title}</p>
                        <p class="text-xs text-gray-500 dark:text-gray-400 truncate">
                          {book.authors?.length > 0 ? book.authors.join(', ') : book.format}
                        </p>
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </Show>

            {/* Empty state */}
            <Show when={books().length === 0 && !loading()}>
              <div class="card p-12 text-center">
                <svg class="w-16 h-16 mx-auto mb-4 text-gray-300 dark:text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
                </svg>
                <h3 class="text-lg font-medium mb-2">Start Reading</h3>
                <p class="text-gray-500 dark:text-gray-400">
                  Open a book file directly or browse a folder to discover books
                </p>
              </div>
            </Show>
          </div>
        </Show>

        {/* Reader View */}
        <Show when={view() === 'reader' && currentBook()}>
          <div
            class="flex flex-col h-full"
            classList={{
              'book-opening': bookOpening(),
              'book-closing': bookClosing(),
            }}
          >
            {/* Reading progress bar at very top */}
            <div
              style={{
                background: readingMode() === 'sepia'
                  ? '#ede0c8'
                  : readingMode() === 'dark'
                    ? '#16213e'
                    : '#f3f4f6',
                height: '3px',
                position: 'relative',
                'flex-shrink': '0',
              }}
            >
              <div
                class="reading-progress-bar"
                style={{
                  width: `${progressPercent()}%`,
                  background: modeStyles().progressBar,
                  position: 'absolute',
                  top: '0',
                  left: '0',
                  bottom: '0',
                  'border-radius': '0 2px 2px 0',
                }}
              />
            </div>

            {/* Reader Header */}
            <div
              class="flex items-center justify-between px-4 py-2 border-b"
              style={{
                background: modeStyles().headerBg,
                'border-color': modeStyles().headerBorder,
                color: modeStyles().text,
                'flex-shrink': '0',
              }}
            >
              <div class="flex items-center gap-4">
                <button
                  class="p-2 rounded-lg transition-colors"
                  style={{
                    color: modeStyles().text,
                  }}
                  onMouseEnter={(e) => {
                    (e.currentTarget as HTMLElement).style.background = modeStyles().hoverBg;
                  }}
                  onMouseLeave={(e) => {
                    (e.currentTarget as HTMLElement).style.background = 'transparent';
                  }}
                  onClick={closeBook}
                  title="Back to library (Esc)"
                >
                  <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
                  </svg>
                </button>
                <div>
                  <h2 class="font-medium truncate max-w-md">{currentBook()!.metadata.title}</h2>
                  <p class="text-xs" style={{ color: modeStyles().mutedText }}>
                    {currentBook()!.metadata.authors?.length > 0
                      ? currentBook()!.metadata.authors.join(', ') + ' \u00B7 '
                      : ''}
                    Chapter {currentChapter() + 1} of {currentBook()!.chapters.length}
                  </p>
                </div>
              </div>

              <div class="flex items-center gap-2">
                {/* Reading mode toggles */}
                <div class="flex items-center gap-1.5 mr-3">
                  <button
                    class={`mode-btn mode-btn-light ${readingMode() === 'light' ? 'active' : ''}`}
                    onClick={() => setReadingMode('light')}
                    title="Light mode"
                  />
                  <button
                    class={`mode-btn mode-btn-sepia ${readingMode() === 'sepia' ? 'active' : ''}`}
                    onClick={() => setReadingMode('sepia')}
                    title="Sepia mode"
                  />
                  <button
                    class={`mode-btn mode-btn-dark ${readingMode() === 'dark' ? 'active' : ''}`}
                    onClick={() => setReadingMode('dark')}
                    title="Dark mode"
                  />
                </div>

                {/* Divider */}
                <div
                  style={{
                    width: '1px',
                    height: '20px',
                    background: modeStyles().headerBorder,
                    'margin-right': '4px',
                  }}
                />

                {/* Font size controls */}
                <button
                  class="p-2 rounded-lg transition-colors"
                  style={{ color: modeStyles().text }}
                  onMouseEnter={(e) => {
                    (e.currentTarget as HTMLElement).style.background = modeStyles().hoverBg;
                  }}
                  onMouseLeave={(e) => {
                    (e.currentTarget as HTMLElement).style.background = 'transparent';
                  }}
                  onClick={() => setFontSize(Math.max(12, fontSize() - 2))}
                  title="Decrease font size"
                >
                  <span class="text-sm font-bold">A-</span>
                </button>
                <span class="text-sm" style={{ color: modeStyles().mutedText }}>
                  {fontSize()}px
                </span>
                <button
                  class="p-2 rounded-lg transition-colors"
                  style={{ color: modeStyles().text }}
                  onMouseEnter={(e) => {
                    (e.currentTarget as HTMLElement).style.background = modeStyles().hoverBg;
                  }}
                  onMouseLeave={(e) => {
                    (e.currentTarget as HTMLElement).style.background = 'transparent';
                  }}
                  onClick={() => setFontSize(Math.min(28, fontSize() + 2))}
                  title="Increase font size"
                >
                  <span class="text-sm font-bold">A+</span>
                </button>

                {/* TOC toggle */}
                <button
                  class="p-2 rounded-lg ml-3 transition-colors"
                  style={{ color: modeStyles().text }}
                  onMouseEnter={(e) => {
                    (e.currentTarget as HTMLElement).style.background = modeStyles().hoverBg;
                  }}
                  onMouseLeave={(e) => {
                    (e.currentTarget as HTMLElement).style.background = 'transparent';
                  }}
                  onClick={() => setShowToc(!showToc())}
                  title="Table of contents"
                >
                  <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h7" />
                  </svg>
                </button>
              </div>
            </div>

            {/* Content area */}
            <div class="flex-1 flex overflow-hidden" style={{ 'min-height': '0' }}>
              {/* TOC Sidebar */}
              <Show when={showToc()}>
                <div
                  class="toc-sidebar w-64 border-r overflow-y-auto flex-shrink-0"
                  style={{
                    background: modeStyles().sidebarBg,
                    'border-color': modeStyles().headerBorder,
                    color: modeStyles().text,
                  }}
                >
                  <div class="p-4">
                    <h3 class="font-medium mb-3">Contents</h3>
                    <div class="space-y-1">
                      <For each={currentBook()!.chapters}>
                        {(chapter, index) => (
                          <button
                            class="w-full text-left px-3 py-2 rounded-lg text-sm transition-colors"
                            style={{
                              background: index() === currentChapter()
                                ? modeStyles().activeBg
                                : 'transparent',
                              color: index() === currentChapter()
                                ? modeStyles().text
                                : modeStyles().mutedText,
                            }}
                            onMouseEnter={(e) => {
                              if (index() !== currentChapter()) {
                                (e.currentTarget as HTMLElement).style.background =
                                  modeStyles().hoverBg;
                              }
                            }}
                            onMouseLeave={(e) => {
                              if (index() !== currentChapter()) {
                                (e.currentTarget as HTMLElement).style.background = 'transparent';
                              }
                            }}
                            onClick={() => goToChapter(index())}
                          >
                            <span class="truncate block">
                              {chapter.title || `Chapter ${index() + 1}`}
                            </span>
                          </button>
                        )}
                      </For>
                    </div>
                  </div>
                </div>
              </Show>

              {/* Reading content with page turn animation */}
              <div
                id="reader-content-scroll"
                class="flex-1 overflow-y-auto"
                style={{
                  background: modeStyles().contentBg,
                  color: modeStyles().text,
                }}
              >
                <div
                  class="max-w-3xl mx-auto px-8 py-12"
                  style={{ 'font-size': `${fontSize()}px` }}
                  classList={{
                    'page-exit-left': pageTransitioning() && pageDirection() === 'left',
                    'page-enter-right': !pageTransitioning() && pageDirection() === 'left',
                    'page-exit-right': pageTransitioning() && pageDirection() === 'right',
                    'page-enter-left': !pageTransitioning() && pageDirection() === 'right',
                  }}
                >
                  <Show when={false}>
                    {/* PDF embed placeholder - convertFileSrc kept for future use */}
                    <iframe src={convertFileSrc('')} title="pdf" />
                  </Show>
                  <Show when={currentBook()!.format !== 'pdf' && currentBook()!.chapters[currentChapter()]}>
                    <h1
                      class="text-2xl font-bold mb-8"
                      style={{
                        color: modeStyles().chapterTitle,
                        'letter-spacing': '-0.02em',
                      }}
                    >
                      {currentBook()!.chapters[currentChapter()].title ||
                        `Chapter ${currentChapter() + 1}`}
                    </h1>
                    <div
                      class={`prose max-w-none ${modeStyles().prose}`}
                      innerHTML={currentBook()!.chapters[currentChapter()].content}
                    />
                  </Show>
                </div>
              </div>
            </div>

            {/* Navigation footer */}
            <div
              class="flex items-center justify-between px-4 py-3 border-t"
              style={{
                background: modeStyles().navBg,
                'border-color': modeStyles().navBorder,
                color: modeStyles().text,
                'flex-shrink': '0',
              }}
            >
              <button
                class="btn nav-btn"
                style={{
                  background: modeStyles().hoverBg,
                  color: modeStyles().text,
                  '--nav-hover-x': '-3px',
                  opacity: currentChapter() === 0 ? '0.35' : '1',
                  cursor: currentChapter() === 0 ? 'not-allowed' : 'pointer',
                } as any}
                onClick={prevChapter}
                disabled={currentChapter() === 0 || pageTransitioning()}
              >
                <span class="flex items-center gap-1.5">
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
                  </svg>
                  Previous
                </span>
              </button>

              <div class="flex items-center gap-3">
                <span class="text-sm" style={{ color: modeStyles().mutedText }}>
                  {currentChapter() + 1} / {currentBook()!.chapters.length}
                </span>
                <span class="text-xs" style={{ color: modeStyles().mutedText }}>
                  ({Math.round(progressPercent())}%)
                </span>
              </div>

              <button
                class="btn nav-btn"
                style={{
                  background: modeStyles().hoverBg,
                  color: modeStyles().text,
                  '--nav-hover-x': '3px',
                  opacity:
                    currentChapter() >= currentBook()!.chapters.length - 1 ? '0.35' : '1',
                  cursor:
                    currentChapter() >= currentBook()!.chapters.length - 1
                      ? 'not-allowed'
                      : 'pointer',
                } as any}
                onClick={nextChapter}
                disabled={
                  currentChapter() >= currentBook()!.chapters.length - 1 || pageTransitioning()
                }
              >
                <span class="flex items-center gap-1.5">
                  Next
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
                  </svg>
                </span>
              </button>
            </div>
          </div>
        </Show>
      </div>
    </>
  );
};

export default Reader;
