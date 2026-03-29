import { Component, createSignal, onMount, onCleanup, For, Show } from 'solid-js';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

// ============================================================================
// Types
// ============================================================================

interface LibraryBook {
  id: string;
  title?: string;
  authors?: string;
  file_path: string;
  format?: string;
  cover_path?: string;
  pages?: number;
  current_position?: string;
  progress: number;
  rating?: number;
  favorite: boolean;
  tags?: string;
  added_at: string;
  last_read_at?: string;
}

interface Collection {
  id: string;
  name: string;
  description?: string;
  color: string;
  book_count: number;
  created_at: string;
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
  cover_base64?: string;
}

type ReadingMode = 'light' | 'dark' | 'sepia';
type LibraryTab = 'all' | 'collections' | 'oreilly';

const PRESET_COLORS = [
  '#0ea5e9', '#8b5cf6', '#ec4899', '#f97316', '#22c55e',
  '#ef4444', '#14b8a6', '#f59e0b', '#6366f1', '#64748b',
];

// ============================================================================
// Component
// ============================================================================

const Reader: Component = () => {
  // Library state
  const [libraryBooks, setLibraryBooks] = createSignal<LibraryBook[]>([]);
  const [collections, setCollections] = createSignal<Collection[]>([]);
  const [libraryTab, setLibraryTab] = createSignal<LibraryTab>('all');
  const [bookSearch, setBookSearch] = createSignal('');

  const filteredBooks = () => {
    const q = bookSearch().toLowerCase().trim();
    if (!q) return libraryBooks();
    return libraryBooks().filter(b =>
      (b.title || '').toLowerCase().includes(q) ||
      (b.authors || '').toLowerCase().includes(q) ||
      (b.file_path || '').toLowerCase().includes(q) ||
      (b.format || '').toLowerCase().includes(q)
    );
  };

  // Old-style file browsing (kept for "Open a Book" and "Browse Folder")
  const [bookPath, setBookPath] = createSignal('');

  // Reader state
  const [currentBook, setCurrentBook] = createSignal<BookContent | null>(null);
  const [currentBookId, setCurrentBookId] = createSignal<string | null>(null);
  const [currentChapter, setCurrentChapter] = createSignal(0);
  const [view, setView] = createSignal<'library' | 'reader'>('library');
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

  // Collection creation form
  const [showNewCollection, setShowNewCollection] = createSignal(false);
  const [newCollectionName, setNewCollectionName] = createSignal('');
  const [newCollectionColor, setNewCollectionColor] = createSignal('#0ea5e9');
  const [creatingCollection, setCreatingCollection] = createSignal(false);

  // Collection detail view
  const [expandedCollection, setExpandedCollection] = createSignal<string | null>(null);
  const [collectionBooks, setCollectionBooks] = createSignal<LibraryBook[]>([]);
  const [loadingCollectionBooks, setLoadingCollectionBooks] = createSignal(false);

  // "Add to Collection" dropdown
  const [addToCollectionBookId, setAddToCollectionBookId] = createSignal<string | null>(null);

  // O'Reilly state
  const [oreillyEmail, setOreillyEmail] = createSignal('');
  const [oreillyPassword, setOreillyPassword] = createSignal('');
  const [oreillyConnected, setOreillyConnected] = createSignal(false);
  const [oreillyConnecting, setOreillyConnecting] = createSignal(false);
  const [oreillyStatus, setOreillyStatus] = createSignal('');

  // Loading metadata (shown while full content loads)
  const [loadingBookMeta, setLoadingBookMeta] = createSignal<{ title: string; author: string } | null>(null);

  // ============================================================================
  // Keyboard navigation
  // ============================================================================

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

  // ============================================================================
  // Lifecycle
  // ============================================================================

  onMount(async () => {
    document.addEventListener('keydown', handleKeyDown);
    // Load persistent library and collections from DB
    await Promise.all([loadLibrary(), loadCollections()]);
  });

  onCleanup(() => {
    document.removeEventListener('keydown', handleKeyDown);
  });

  // ============================================================================
  // Library persistence
  // ============================================================================

  const loadLibrary = async () => {
    try {
      const books = await invoke<LibraryBook[]>('reader_get_library');
      setLibraryBooks(books);
    } catch (e) {
      console.error('Failed to load library:', e);
    }
  };

  const loadCollections = async () => {
    try {
      const cols = await invoke<Collection[]>('reader_list_collections');
      setCollections(cols);
    } catch (e) {
      console.error('Failed to load collections:', e);
    }
  };

  // ============================================================================
  // Book opening / importing
  // ============================================================================

  const browseForBook = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: 'Books', extensions: ['epub', 'pdf', 'txt', 'md', 'markdown'] }],
      });
      if (selected && typeof selected === 'string') {
        setBookPath(selected);
        await openBookByPath(selected);
      }
    } catch (e) {
      console.error('Failed to open file dialog:', e);
      alert(`Error opening file dialog: ${e}`);
    }
  };

  const browseForLibraryFolder = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
      });
      if (selected && typeof selected === 'string') {
        setLoading(true);
        try {
          const imported = await invoke<LibraryBook[]>('reader_scan_directory', {
            path: selected,
          });
          // Refresh full library after scan
          await loadLibrary();
          if (imported.length === 0) {
            alert('No supported book files found in that directory.');
          }
        } catch (e) {
          console.error('Failed to scan directory:', e);
          alert(`Error scanning directory: ${e}`);
        } finally {
          setLoading(false);
        }
      }
    } catch (e) {
      console.error('Failed to open folder dialog:', e);
      alert(`Error: ${e}`);
    }
  };

  const openBookByPath = async (path: string, cardIndex?: number) => {
    if (loading() || bookClosing()) return;
    setLoading(true);
    try {
      // Import book to DB for metadata
      const imported = await invoke<LibraryBook>('reader_import_book', { path });
      setCurrentBookId(imported.id);

      // Load full content (single phase - more reliable)
      const content = await invoke<BookContent>('reader_open_book', { path });
      setCurrentBook(content);

      // Restore position
      const startChapter = imported.current_position ? parseInt(imported.current_position, 10) || 0 : 0;
      setCurrentChapter(Math.min(startChapter, Math.max(0, content.chapters.length - 1)));

      // Show reader with open animation
      if (cardIndex !== undefined) setOpeningCardIndex(cardIndex);
      setBookOpening(true);
      setView('reader');
      setTimeout(() => { setBookOpening(false); setOpeningCardIndex(null); }, 500);

      loadLibrary();
    } catch (e) {
      console.error('Failed to open book:', e);
      alert(`Error opening book: ${e}`);
      setView('library');
    } finally {
      setLoading(false);
    }
  };

  const openBookDirect = async () => {
    const path = bookPath().trim();
    if (!path) return;
    await openBookByPath(path);
  };

  const openLibraryBook = async (book: LibraryBook, cardIndex?: number) => {
    await openBookByPath(book.file_path, cardIndex);
  };

  const closeBook = () => {
    if (bookClosing()) return;
    setBookClosing(true);
    setTimeout(() => {
      setCurrentBook(null);
      setCurrentBookId(null);
      setLoadingBookMeta(null);
      setView('library');
      setBookClosing(false);
      setShowToc(false);
      // Refresh library to show updated progress
      loadLibrary();
    }, 350);
  };

  // ============================================================================
  // Chapter navigation with progress persistence
  // ============================================================================

  const saveProgress = async (chapterIdx: number) => {
    const bookId = currentBookId();
    const book = currentBook();
    if (!bookId || !book) return;

    const progress = ((chapterIdx + 1) / book.chapters.length) * 100;
    const position = String(chapterIdx);

    try {
      await invoke('reader_update_progress', {
        bookId,
        progress,
        position,
      });
    } catch (e) {
      console.error('Failed to save progress:', e);
    }
  };

  const nextChapter = () => {
    const book = currentBook();
    if (!book || currentChapter() >= book.chapters.length - 1) return;
    if (pageTransitioning()) return;

    setPageDirection('left');
    setPageTransitioning(true);

    const newChapter = currentChapter() + 1;

    setTimeout(() => {
      setCurrentChapter(newChapter);
      const contentEl = document.getElementById('reader-content-scroll');
      if (contentEl) contentEl.scrollTop = 0;
      saveProgress(newChapter);
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

    const newChapter = currentChapter() - 1;

    setTimeout(() => {
      setCurrentChapter(newChapter);
      const contentEl = document.getElementById('reader-content-scroll');
      if (contentEl) contentEl.scrollTop = 0;
      saveProgress(newChapter);
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
      saveProgress(index);
    }, 300);

    setTimeout(() => {
      setPageTransitioning(false);
    }, 600);
  };

  // ============================================================================
  // Card 3D tilt
  // ============================================================================

  const handleCardMouseMove = (e: MouseEvent, bookId: string) => {
    const card = e.currentTarget as HTMLElement;
    const rect = card.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const centerX = rect.width / 2;
    const centerY = rect.height / 2;

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

  // ============================================================================
  // Collections
  // ============================================================================

  const createCollection = async () => {
    const name = newCollectionName().trim();
    if (!name) return;
    setCreatingCollection(true);
    try {
      await invoke<Collection>('reader_create_collection', {
        name,
        color: newCollectionColor(),
        description: null,
      });
      setNewCollectionName('');
      setNewCollectionColor('#0ea5e9');
      setShowNewCollection(false);
      await loadCollections();
    } catch (e) {
      console.error('Failed to create collection:', e);
      alert(`Error: ${e}`);
    } finally {
      setCreatingCollection(false);
    }
  };

  const deleteCollection = async (collectionId: string) => {
    if (!confirm('Delete this collection? Books will not be removed from your library.')) return;
    try {
      await invoke('reader_delete_collection', { collectionId });
      if (expandedCollection() === collectionId) {
        setExpandedCollection(null);
        setCollectionBooks([]);
      }
      await loadCollections();
    } catch (e) {
      console.error('Failed to delete collection:', e);
    }
  };

  const expandCollection = async (collectionId: string) => {
    if (expandedCollection() === collectionId) {
      setExpandedCollection(null);
      setCollectionBooks([]);
      return;
    }
    setExpandedCollection(collectionId);
    setLoadingCollectionBooks(true);
    try {
      const books = await invoke<LibraryBook[]>('reader_get_collection_books', { collectionId });
      setCollectionBooks(books);
    } catch (e) {
      console.error('Failed to load collection books:', e);
      setCollectionBooks([]);
    } finally {
      setLoadingCollectionBooks(false);
    }
  };

  const addBookToCollection = async (collectionId: string, bookId: string) => {
    try {
      await invoke('reader_add_to_collection', { collectionId, bookId });
      setAddToCollectionBookId(null);
      await loadCollections();
      // Refresh expanded collection if it matches
      if (expandedCollection() === collectionId) {
        const books = await invoke<LibraryBook[]>('reader_get_collection_books', { collectionId });
        setCollectionBooks(books);
      }
    } catch (e) {
      console.error('Failed to add book to collection:', e);
    }
  };

  const removeBookFromCollection = async (collectionId: string, bookId: string) => {
    try {
      await invoke('reader_remove_from_collection', { collectionId, bookId });
      await loadCollections();
      if (expandedCollection() === collectionId) {
        const books = await invoke<LibraryBook[]>('reader_get_collection_books', { collectionId });
        setCollectionBooks(books);
      }
    } catch (e) {
      console.error('Failed to remove book from collection:', e);
    }
  };

  // ============================================================================
  // Derived values
  // ============================================================================

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
      default:
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

  // ============================================================================
  // Render helpers for book cards
  // ============================================================================

  const renderBookCard = (
    book: LibraryBook,
    index: number,
    options?: { showRemoveFromCollection?: string }
  ) => {
    const displayTitle = book.title || book.file_path.split('/').pop() || 'Untitled';
    const displayAuthors = book.authors || book.format?.toUpperCase() || '';

    return (
      <div
        class="book-card-3d cursor-pointer relative group"
        classList={{
          'card-opening': openingCardIndex() === index,
          'pointer-events-none opacity-60': loading(),
        }}
        onClick={() => !loading() && openLibraryBook(book, index)}
        onMouseMove={(e) => handleCardMouseMove(e, book.id)}
        onMouseLeave={handleCardMouseLeave}
      >
        <div
          class="book-card-inner card p-3 relative overflow-hidden"
          style={{
            transform:
              hoveredCard() === book.id
                ? `rotateX(${tiltX()}deg) rotateY(${tiltY()}deg) translateZ(8px)`
                : 'rotateX(0deg) rotateY(0deg) translateZ(0px)',
          }}
        >
          <div class="book-cover-shine" />
          <div class="aspect-[2/3] bg-gradient-to-br from-minion-100 to-minion-200 dark:from-minion-900 dark:to-minion-800 rounded-lg mb-2 flex items-center justify-center relative overflow-hidden">
            <Show when={book.cover_path} fallback={
              <svg
                class="w-12 h-12 text-minion-500"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="1.5"
                  d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253"
                />
              </svg>
            }>
              <img
                src={book.cover_path!}
                alt={displayTitle}
                class="w-full h-full object-cover rounded-lg"
                onError={(e) => { (e.target as HTMLImageElement).style.display = 'none'; }}
              />
            </Show>
            {/* Progress indicator overlay */}
            <Show when={book.progress > 0}>
              <div
                class="absolute bottom-0 left-0 right-0 h-1 bg-black/10"
                style={{ 'border-radius': '0 0 0.5rem 0.5rem' }}
              >
                <div
                  class="h-full bg-sky-500 rounded-bl-lg"
                  style={{
                    width: `${Math.min(book.progress, 100)}%`,
                    'border-radius':
                      book.progress >= 100 ? '0 0 0.5rem 0.5rem' : '0 0 0 0.5rem',
                  }}
                />
              </div>
            </Show>
          </div>
          <p class="font-medium text-sm truncate" title={displayTitle}>
            {displayTitle}
          </p>
          <p class="text-xs text-gray-500 dark:text-gray-400 truncate">{displayAuthors}</p>
          {/* Progress text */}
          <Show when={book.progress > 0}>
            <p class="text-xs text-sky-600 dark:text-sky-400 mt-0.5">
              {Math.round(book.progress)}% read
            </p>
          </Show>
        </div>

        {/* Add to Collection button */}
        <div
          class="absolute top-1 right-1 opacity-0 group-hover:opacity-100 transition-opacity z-10"
          onClick={(e) => e.stopPropagation()}
        >
          <Show
            when={!options?.showRemoveFromCollection}
            fallback={
              <button
                class="w-6 h-6 rounded-full bg-red-500 text-white flex items-center justify-center text-xs shadow hover:bg-red-600 transition-colors"
                title="Remove from collection"
                onClick={(e) => {
                  e.stopPropagation();
                  removeBookFromCollection(options!.showRemoveFromCollection!, book.id);
                }}
              >
                <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M6 18L18 6M6 6l12 12"
                  />
                </svg>
              </button>
            }
          >
            <button
              class="w-6 h-6 rounded-full bg-white dark:bg-gray-700 border border-gray-200 dark:border-gray-600 text-gray-600 dark:text-gray-300 flex items-center justify-center text-xs shadow hover:bg-sky-50 dark:hover:bg-sky-900 hover:border-sky-300 transition-colors"
              title="Add to collection"
              onClick={(e) => {
                e.stopPropagation();
                setAddToCollectionBookId(
                  addToCollectionBookId() === book.id ? null : book.id
                );
              }}
            >
              <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M12 4v16m8-8H4"
                />
              </svg>
            </button>
            {/* Collection dropdown */}
            <Show when={addToCollectionBookId() === book.id}>
              <div class="absolute right-0 top-8 w-48 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-600 rounded-lg shadow-lg py-1 z-20">
                <Show
                  when={collections().length > 0}
                  fallback={
                    <p class="px-3 py-2 text-xs text-gray-500">
                      No collections yet. Create one first.
                    </p>
                  }
                >
                  <For each={collections()}>
                    {(col) => (
                      <button
                        class="w-full text-left px-3 py-2 text-sm hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-2 transition-colors"
                        onClick={(e) => {
                          e.stopPropagation();
                          addBookToCollection(col.id, book.id);
                        }}
                      >
                        <span
                          class="w-3 h-3 rounded-full flex-shrink-0"
                          style={{ background: col.color }}
                        />
                        <span class="truncate">{col.name}</span>
                      </button>
                    )}
                  </For>
                </Show>
              </div>
            </Show>
          </Show>
        </div>
      </div>
    );
  };

  // ============================================================================
  // Render
  // ============================================================================

  return (
    <>
      <style>{`
        /* ============================================================
         * Apple Books-style CSS
         * ============================================================ */

        /* Book card - realistic 3D book with spine, shadow, hover lift */
        .book-card-3d {
          perspective: 1200px;
          transform-style: preserve-3d;
        }

        .book-card-inner {
          transition: transform 0.4s cubic-bezier(0.25, 0.46, 0.45, 0.94), box-shadow 0.4s ease;
          transform-style: preserve-3d;
          will-change: transform;
          border-radius: 4px;
          /* Realistic book shadow - mimics a physical book on a surface */
          box-shadow:
            -4px 4px 6px rgba(0,0,0,0.08),
            0 1px 3px rgba(0,0,0,0.06);
        }

        .book-card-inner:hover {
          transform: translateY(-8px) scale(1.02);
          box-shadow:
            -6px 20px 40px rgba(0,0,0,0.2),
            -3px 8px 16px rgba(0,0,0,0.12),
            0 2px 4px rgba(0,0,0,0.06);
        }

        /* Glossy shine on hover - like a book jacket */
        .book-card-3d .book-cover-shine {
          position: absolute;
          inset: 0;
          border-radius: 4px;
          background: linear-gradient(
            135deg,
            transparent 30%,
            rgba(255, 255, 255, 0.08) 42%,
            rgba(255, 255, 255, 0.16) 48%,
            rgba(255, 255, 255, 0.08) 54%,
            transparent 66%
          );
          pointer-events: none;
          opacity: 0;
          transition: opacity 0.5s ease;
          z-index: 5;
        }

        .book-card-inner:hover .book-cover-shine {
          opacity: 1;
        }

        /* Book spine - thick left edge with gradient to look 3D */
        .book-card-inner::before {
          content: '';
          position: absolute;
          left: 0;
          top: 0;
          bottom: 0;
          width: 6px;
          background: linear-gradient(
            to right,
            rgba(0,0,0,0.25),
            rgba(0,0,0,0.08) 40%,
            rgba(255,255,255,0.05) 60%,
            transparent
          );
          border-radius: 4px 0 0 4px;
          z-index: 3;
        }

        /* Page edge effect - white lines on the right (like stacked pages) */
        .book-card-inner::after {
          content: '';
          position: absolute;
          right: 0;
          top: 8px;
          bottom: 8px;
          width: 4px;
          background: repeating-linear-gradient(
            to bottom,
            rgba(0,0,0,0.03) 0px,
            rgba(0,0,0,0.03) 1px,
            rgba(255,255,255,0.6) 1px,
            rgba(255,255,255,0.6) 2px
          );
          border-radius: 0 4px 4px 0;
          z-index: 3;
        }

        /* Page turn animations - Apple Books style smooth slide with depth */
        @keyframes pageExitLeft {
          0% {
            transform: perspective(1200px) translateX(0) rotateY(0deg);
            opacity: 1;
            transform-origin: left center;
          }
          100% {
            transform: perspective(1200px) translateX(-30px) rotateY(15deg);
            opacity: 0;
            transform-origin: left center;
          }
        }

        @keyframes pageEnterRight {
          0% {
            transform: perspective(1200px) translateX(50px) rotateY(-10deg);
            opacity: 0;
            transform-origin: right center;
          }
          100% {
            transform: perspective(1200px) translateX(0) rotateY(0deg);
            opacity: 1;
            transform-origin: right center;
          }
        }

        @keyframes pageExitRight {
          0% {
            transform: perspective(1200px) translateX(0) rotateY(0deg);
            opacity: 1;
            transform-origin: right center;
          }
          100% {
            transform: perspective(1200px) translateX(30px) rotateY(-15deg);
            opacity: 0;
            transform-origin: right center;
          }
        }

        @keyframes pageEnterLeft {
          0% {
            transform: perspective(1200px) translateX(-50px) rotateY(10deg);
            opacity: 0;
            transform-origin: left center;
          }
          100% {
            transform: perspective(1200px) translateX(0) rotateY(0deg);
            opacity: 1;
            transform-origin: left center;
          }
        }

        .page-exit-left {
          animation: pageExitLeft 0.35s cubic-bezier(0.4, 0, 0.2, 1) forwards;
        }

        .page-enter-right {
          animation: pageEnterRight 0.4s cubic-bezier(0.16, 1, 0.3, 1) forwards;
        }

        .page-exit-right {
          animation: pageExitRight 0.35s cubic-bezier(0.4, 0, 0.2, 1) forwards;
        }

        .page-enter-left {
          animation: pageEnterLeft 0.4s cubic-bezier(0.16, 1, 0.3, 1) forwards;
        }

        /* Book open - zooms from card position with a page-unfold feel */
        @keyframes bookOpen {
          0% {
            transform: scale(0.4) rotateY(-20deg);
            opacity: 0;
            filter: blur(8px);
          }
          50% {
            transform: scale(0.9) rotateY(-5deg);
            opacity: 0.8;
            filter: blur(1px);
          }
          80% {
            transform: scale(1.01) rotateY(0deg);
            opacity: 1;
            filter: blur(0);
          }
          100% {
            transform: scale(1) rotateY(0deg);
            opacity: 1;
            filter: blur(0);
          }
        }

        @keyframes bookClose {
          0% {
            transform: scale(1) rotateY(0deg);
            opacity: 1;
            filter: blur(0);
          }
          40% {
            transform: scale(0.95) rotateY(5deg);
            opacity: 0.9;
            filter: blur(1px);
          }
          100% {
            transform: scale(0.4) rotateY(20deg);
            opacity: 0;
            filter: blur(8px);
          }
        }

        .book-opening {
          animation: bookOpen 0.6s cubic-bezier(0.16, 1, 0.3, 1) forwards;
        }

        .book-closing {
          animation: bookClose 0.4s cubic-bezier(0.4, 0, 0.2, 1) forwards;
        }

        /* Opening card pulse - the card being opened shrinks as the reader expands */
        @keyframes cardShrinkAway {
          0% { transform: scale(1); opacity: 1; }
          50% { transform: scale(0.85); opacity: 0.5; }
          100% { transform: scale(0.7); opacity: 0; }
        }

        .card-opening {
          animation: cardShrinkAway 0.5s cubic-bezier(0.4, 0, 0.2, 1) forwards;
          pointer-events: none;
        }

        /* Reading progress bar */
        .reading-progress-bar {
          height: 3px;
          transition: width 0.4s cubic-bezier(0.4, 0, 0.2, 1);
        }

        /* Loading overlay for book opening */
        @keyframes loadingPulse {
          0%, 100% { opacity: 0.4; }
          50% { opacity: 1; }
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

        /* Library tab styles */
        .lib-tab {
          padding: 0.5rem 1rem;
          border-radius: 0.5rem;
          font-size: 0.875rem;
          font-weight: 500;
          cursor: pointer;
          transition: background 0.15s, color 0.15s;
          border: none;
          background: transparent;
          color: #6b7280;
        }

        .lib-tab:hover {
          background: #f3f4f6;
          color: #374151;
        }

        .lib-tab.active {
          background: #0ea5e9;
          color: white;
        }

        .dark .lib-tab:hover {
          background: #374151;
          color: #d1d5db;
        }

        .dark .lib-tab.active {
          background: #0ea5e9;
          color: white;
        }
      `}</style>

      <div class="h-full flex flex-col">
        {/* ================================================================ */}
        {/* LIBRARY VIEW                                                     */}
        {/* ================================================================ */}
        <Show when={view() === 'library'}>
          <div class="p-6 flex-1 overflow-auto relative">
            {/* Loading overlay when opening a book */}
            <Show when={loading()}>
              <div class="absolute inset-0 z-50 flex items-center justify-center bg-white/80 dark:bg-gray-900/80 backdrop-blur-sm">
                <div class="text-center">
                  <div class="w-12 h-12 mx-auto mb-3 rounded-full border-3 border-gray-200 dark:border-gray-700 border-t-minion-500" style={{ 'border-width': '3px', animation: 'spin 1s linear infinite' }} />
                  <p class="text-sm font-medium text-gray-600 dark:text-gray-300" style={{ animation: 'loadingPulse 1.5s ease infinite' }}>Opening book...</p>
                </div>
              </div>
            </Show>
            {/* Header */}
            <div class="flex items-center justify-between mb-6">
              <h1 class="text-2xl font-bold">Book Reader</h1>
              <div class="flex gap-2">
                <button
                  class="btn btn-secondary text-sm"
                  onClick={browseForLibraryFolder}
                  disabled={loading()}
                  title="Scan a folder for books and import them"
                >
                  <svg
                    class="w-4 h-4 mr-1.5 inline-block"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
                    />
                  </svg>
                  Import Folder
                </button>
                <button
                  class="btn btn-primary text-sm"
                  onClick={browseForBook}
                  disabled={loading()}
                  title="Open a single book file"
                >
                  <svg
                    class="w-4 h-4 mr-1.5 inline-block"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M12 6v6m0 0v6m0-6h6m-6 0H6"
                    />
                  </svg>
                  Open Book
                </button>
              </div>
            </div>

            {/* Open a book by path (compact) */}
            <div class="card p-3 mb-5">
              <div class="flex gap-2">
                <input
                  type="text"
                  class="input flex-1 text-sm"
                  placeholder="Enter path to book file (e.g., /path/to/book.epub)"
                  value={bookPath()}
                  onInput={(e) => setBookPath(e.currentTarget.value)}
                  onKeyPress={(e) => e.key === 'Enter' && openBookDirect()}
                />
                <button
                  class="btn btn-primary text-sm"
                  onClick={openBookDirect}
                  disabled={loading() || !bookPath().trim()}
                >
                  {loading() ? 'Opening...' : 'Open'}
                </button>
              </div>
              <p class="text-xs text-gray-500 mt-1.5">
                Supported formats: EPUB, PDF, TXT, Markdown
              </p>
            </div>

            {/* Library tabs */}
            <div class="flex items-center gap-1 mb-5">
              <button
                class={`lib-tab ${libraryTab() === 'all' ? 'active' : ''}`}
                onClick={() => setLibraryTab('all')}
              >
                All Books
                <Show when={libraryBooks().length > 0}>
                  <span class="ml-1.5 text-xs opacity-75">({libraryBooks().length})</span>
                </Show>
              </button>
              <button
                class={`lib-tab ${libraryTab() === 'collections' ? 'active' : ''}`}
                onClick={() => setLibraryTab('collections')}
              >
                Collections
                <Show when={collections().length > 0}>
                  <span class="ml-1.5 text-xs opacity-75">({collections().length})</span>
                </Show>
              </button>
              <button
                class={`lib-tab ${libraryTab() === 'oreilly' ? 'active' : ''}`}
                onClick={() => setLibraryTab('oreilly')}
              >
                O'Reilly
              </button>
            </div>

            {/* ============================================================ */}
            {/* TAB: All Books                                               */}
            {/* ============================================================ */}
            <Show when={libraryTab() === 'all'}>
              <Show
                when={libraryBooks().length > 0}
                fallback={
                  <div class="card p-12 text-center">
                    <svg
                      class="w-16 h-16 mx-auto mb-4 text-gray-300 dark:text-gray-600"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="1.5"
                        d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253"
                      />
                    </svg>
                    <h3 class="text-lg font-medium mb-2">Your Library is Empty</h3>
                    <p class="text-gray-500 dark:text-gray-400 mb-4">
                      Open a book file or import a folder to get started.
                    </p>
                    <div class="flex gap-3 justify-center">
                      <button class="btn btn-primary" onClick={browseForBook}>
                        Open a Book
                      </button>
                      <button class="btn btn-secondary" onClick={browseForLibraryFolder}>
                        Import Folder
                      </button>
                    </div>
                  </div>
                }
              >
                {/* Search bar */}
                <div class="mb-4">
                  <div class="relative">
                    <svg class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                    </svg>
                    <input
                      type="text"
                      class="input w-full pl-10"
                      placeholder="Search books by title, author, or format..."
                      value={bookSearch()}
                      onInput={(e) => setBookSearch(e.currentTarget.value)}
                    />
                    <Show when={bookSearch()}>
                      <button
                        class="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600"
                        onClick={() => setBookSearch('')}
                      >
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                        </svg>
                      </button>
                    </Show>
                  </div>
                  <Show when={bookSearch()}>
                    <p class="text-xs text-gray-400 mt-1">{filteredBooks().length} of {libraryBooks().length} books</p>
                  </Show>
                </div>

                <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-5">
                  <For each={filteredBooks()}>
                    {(book, index) => renderBookCard(book, index())}
                  </For>
                </div>
              </Show>
            </Show>

            {/* ============================================================ */}
            {/* TAB: Collections                                             */}
            {/* ============================================================ */}
            <Show when={libraryTab() === 'collections'}>
              {/* New Collection button / form */}
              <div class="mb-5">
                <Show
                  when={showNewCollection()}
                  fallback={
                    <button
                      class="btn btn-secondary text-sm"
                      onClick={() => setShowNewCollection(true)}
                    >
                      <svg
                        class="w-4 h-4 mr-1.5 inline-block"
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          stroke-width="2"
                          d="M12 4v16m8-8H4"
                        />
                      </svg>
                      New Collection
                    </button>
                  }
                >
                  <div class="card p-4">
                    <h4 class="font-medium mb-3">Create Collection</h4>
                    <div class="flex gap-3 items-end">
                      <div class="flex-1">
                        <label class="block text-xs text-gray-500 mb-1">Name</label>
                        <input
                          type="text"
                          class="input w-full text-sm"
                          placeholder="e.g., Computer Science, Fiction..."
                          value={newCollectionName()}
                          onInput={(e) => setNewCollectionName(e.currentTarget.value)}
                          onKeyPress={(e) => e.key === 'Enter' && createCollection()}
                        />
                      </div>
                      <div>
                        <label class="block text-xs text-gray-500 mb-1">Color</label>
                        <div class="flex gap-1.5">
                          <For each={PRESET_COLORS}>
                            {(color) => (
                              <button
                                class="w-6 h-6 rounded-full border-2 transition-transform hover:scale-110"
                                style={{
                                  background: color,
                                  'border-color':
                                    newCollectionColor() === color
                                      ? '#1f2937'
                                      : 'transparent',
                                }}
                                onClick={() => setNewCollectionColor(color)}
                              />
                            )}
                          </For>
                        </div>
                      </div>
                      <button
                        class="btn btn-primary text-sm"
                        onClick={createCollection}
                        disabled={!newCollectionName().trim() || creatingCollection()}
                      >
                        {creatingCollection() ? 'Creating...' : 'Create'}
                      </button>
                      <button
                        class="btn btn-secondary text-sm"
                        onClick={() => {
                          setShowNewCollection(false);
                          setNewCollectionName('');
                        }}
                      >
                        Cancel
                      </button>
                    </div>
                  </div>
                </Show>
              </div>

              {/* Collection cards */}
              <Show
                when={collections().length > 0}
                fallback={
                  <div class="card p-12 text-center">
                    <svg
                      class="w-12 h-12 mx-auto mb-3 text-gray-300 dark:text-gray-600"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="1.5"
                        d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"
                      />
                    </svg>
                    <h3 class="text-lg font-medium mb-1">No Collections Yet</h3>
                    <p class="text-gray-500 dark:text-gray-400">
                      Create a collection to organize your books.
                    </p>
                  </div>
                }
              >
                <div class="space-y-3">
                  <For each={collections()}>
                    {(col) => (
                      <div class="card overflow-hidden">
                        {/* Collection header */}
                        <div
                          class="flex items-center gap-3 p-4 cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors"
                          onClick={() => expandCollection(col.id)}
                        >
                          <div
                            class="w-1.5 self-stretch rounded-full flex-shrink-0"
                            style={{ background: col.color }}
                          />
                          <div class="flex-1 min-w-0">
                            <h3 class="font-medium">{col.name}</h3>
                            <p class="text-sm text-gray-500 dark:text-gray-400">
                              {col.book_count} {col.book_count === 1 ? 'book' : 'books'}
                            </p>
                          </div>
                          <button
                            class="p-1.5 rounded-lg text-gray-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
                            title="Delete collection"
                            onClick={(e) => {
                              e.stopPropagation();
                              deleteCollection(col.id);
                            }}
                          >
                            <svg
                              class="w-4 h-4"
                              fill="none"
                              stroke="currentColor"
                              viewBox="0 0 24 24"
                            >
                              <path
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                stroke-width="2"
                                d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
                              />
                            </svg>
                          </button>
                          <svg
                            class="w-5 h-5 text-gray-400 transition-transform"
                            classList={{
                              'rotate-180': expandedCollection() === col.id,
                            }}
                            fill="none"
                            stroke="currentColor"
                            viewBox="0 0 24 24"
                          >
                            <path
                              stroke-linecap="round"
                              stroke-linejoin="round"
                              stroke-width="2"
                              d="M19 9l-7 7-7-7"
                            />
                          </svg>
                        </div>

                        {/* Expanded collection books */}
                        <Show when={expandedCollection() === col.id}>
                          <div
                            class="border-t border-gray-100 dark:border-gray-700 p-4"
                            style={{ 'border-top-color': col.color + '30' }}
                          >
                            <Show
                              when={!loadingCollectionBooks()}
                              fallback={
                                <p class="text-sm text-gray-500 py-4 text-center">
                                  Loading books...
                                </p>
                              }
                            >
                              <Show
                                when={collectionBooks().length > 0}
                                fallback={
                                  <p class="text-sm text-gray-500 py-4 text-center">
                                    No books in this collection yet. Use the "+" button on book
                                    cards to add them.
                                  </p>
                                }
                              >
                                <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4">
                                  <For each={collectionBooks()}>
                                    {(book, index) =>
                                      renderBookCard(book, index(), {
                                        showRemoveFromCollection: col.id,
                                      })
                                    }
                                  </For>
                                </div>
                              </Show>
                            </Show>
                          </div>
                        </Show>
                      </div>
                    )}
                  </For>
                </div>
              </Show>
            </Show>

            {/* ============================================================ */}
            {/* TAB: O'Reilly                                                */}
            {/* ============================================================ */}
            <Show when={libraryTab() === 'oreilly'}>
              <div class="max-w-lg mx-auto">
                <div class="card p-6">
                  <div class="flex items-start gap-4 mb-5">
                    <div class="w-12 h-12 rounded-xl bg-gradient-to-br from-red-500 to-red-700 flex items-center justify-center flex-shrink-0">
                      <svg
                        class="w-7 h-7 text-white"
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          stroke-width="1.5"
                          d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253"
                        />
                      </svg>
                    </div>
                    <div class="flex-1">
                      <div class="flex items-center gap-2 mb-1">
                        <h3 class="font-semibold text-lg">O'Reilly Learning</h3>
                        <span class="px-2 py-0.5 rounded-full bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400 text-xs font-medium">
                          Beta
                        </span>
                      </div>
                      <p class="text-sm text-gray-500 dark:text-gray-400">
                        Connect to O'Reilly Learning (Safari Books Online) to browse and download
                        books directly to your library.
                      </p>
                    </div>
                  </div>

                  <Show when={!oreillyConnected()}>
                    {/* Login options */}
                    <div class="space-y-3 mb-5">
                      <p class="text-sm text-gray-600 dark:text-gray-300 mb-3">
                        Choose how to connect to your O'Reilly account:
                      </p>

                      {/* Option 1: Use Chrome session */}
                      <button
                        class="btn w-full flex items-center gap-3 p-4 bg-gray-50 dark:bg-gray-800 border-2 border-gray-200 dark:border-gray-700 hover:border-minion-400 dark:hover:border-minion-500 rounded-xl transition-colors text-left"
                        disabled={oreillyConnecting()}
                        onClick={async () => {
                          setOreillyConnecting(true);
                          setOreillyStatus('Reading Chrome cookies for oreilly.com...');
                          try {
                            const result = await invoke<{ success: boolean; message: string }>('oreilly_connect_chrome');
                            if (result.success) {
                              setOreillyConnected(true);
                              setOreillyStatus('Connected via Chrome session!');
                            } else {
                              setOreillyStatus(result.message);
                            }
                          } catch (e) {
                            setOreillyStatus(`Failed: ${e}. Make sure you're logged into O'Reilly in Chrome.`);
                          } finally {
                            setOreillyConnecting(false);
                          }
                        }}
                      >
                        <div class="w-10 h-10 rounded-lg bg-blue-100 dark:bg-blue-900/30 flex items-center justify-center flex-shrink-0">
                          <svg class="w-6 h-6 text-blue-600" viewBox="0 0 24 24" fill="currentColor">
                            <circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="1.5"/>
                            <circle cx="12" cy="12" r="4" fill="currentColor" opacity="0.6"/>
                          </svg>
                        </div>
                        <div class="flex-1">
                          <p class="font-medium text-sm">Use Chrome Session</p>
                          <p class="text-xs text-gray-500 dark:text-gray-400">
                            Recommended. Uses your existing Chrome login (ACM SSO supported).
                          </p>
                        </div>
                        <Show when={oreillyConnecting()}>
                          <div class="w-5 h-5 rounded-full border-2 border-minion-200 border-t-minion-500" style={{ animation: 'spin 1s linear infinite' }} />
                        </Show>
                      </button>

                      {/* Option 2: Sign in with SSO via MINION browser window */}
                      <button
                        class="btn w-full flex items-center gap-3 p-4 bg-gray-50 dark:bg-gray-800 border-2 border-gray-200 dark:border-gray-700 hover:border-minion-400 dark:hover:border-minion-500 rounded-xl transition-colors text-left"
                        disabled={oreillyConnecting()}
                        onClick={async () => {
                          setOreillyConnecting(true);
                          setOreillyStatus('Opening O\'Reilly login in MINION...');
                          try {
                            await invoke('oreilly_open_browser');
                            setOreillyStatus('Complete the login in the MINION browser window. After SSO login, close that window and click "Use Chrome Session" above.');
                          } catch (e) {
                            setOreillyStatus(`Failed: ${e}`);
                          } finally {
                            setOreillyConnecting(false);
                          }
                        }}
                      >
                        <div class="w-10 h-10 rounded-lg bg-red-100 dark:bg-red-900/30 flex items-center justify-center flex-shrink-0">
                          <svg class="w-6 h-6 text-red-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
                          </svg>
                        </div>
                        <div class="flex-1">
                          <p class="font-medium text-sm">Sign in with SSO (ACM/Institutional)</p>
                          <p class="text-xs text-gray-500 dark:text-gray-400">
                            Opens O'Reilly login in a MINION browser window with full SSO redirect support.
                          </p>
                        </div>
                        <Show when={oreillyConnecting()}>
                          <div class="w-5 h-5 rounded-full border-2 border-minion-200 border-t-minion-500" style={{ animation: 'spin 1s linear infinite' }} />
                        </Show>
                      </button>

                      {/* Option 3: Manual email/password */}
                      <details class="mt-2">
                        <summary class="text-xs text-gray-400 cursor-pointer hover:text-gray-600">
                          Manual login (email + password, no SSO)
                        </summary>
                        <div class="mt-3 space-y-2">
                          <input
                            type="email"
                            class="input w-full text-sm"
                            placeholder="your-email@example.com"
                            value={oreillyEmail()}
                            onInput={(e) => setOreillyEmail(e.currentTarget.value)}
                          />
                          <input
                            type="password"
                            class="input w-full text-sm"
                            placeholder="Password"
                            value={oreillyPassword()}
                            onInput={(e) => setOreillyPassword(e.currentTarget.value)}
                          />
                          <button
                            class="btn btn-secondary w-full text-sm"
                            disabled={!oreillyEmail().trim() || !oreillyPassword().trim() || oreillyConnecting()}
                            onClick={async () => {
                              setOreillyConnecting(true);
                              setOreillyStatus('Logging in...');
                              try {
                                const result = await invoke<{ success: boolean; message: string }>('oreilly_connect_manual', {
                                  email: oreillyEmail(),
                                  password: oreillyPassword(),
                                });
                                if (result.success) {
                                  setOreillyConnected(true);
                                  setOreillyStatus('Connected!');
                                } else {
                                  setOreillyStatus(result.message);
                                }
                              } catch (e) {
                                setOreillyStatus(`${e}`);
                              } finally {
                                setOreillyConnecting(false);
                              }
                            }}
                          >
                            Sign In
                          </button>
                        </div>
                      </details>
                    </div>

                    {/* Status message */}
                    <Show when={oreillyStatus()}>
                      <div class={`mt-3 p-3 rounded-lg text-sm ${
                        oreillyStatus().includes('Failed') || oreillyStatus().includes('failed')
                          ? 'bg-red-50 dark:bg-red-900/10 text-red-700 dark:text-red-300'
                          : 'bg-minion-50 dark:bg-minion-900/10 text-minion-700 dark:text-minion-300'
                      }`}>
                        {oreillyStatus()}
                      </div>
                    </Show>
                  </Show>

                  <Show when={oreillyConnected()}>
                    <div class="p-4 bg-green-50 dark:bg-green-900/10 rounded-lg border border-green-200 dark:border-green-800 mb-4">
                      <div class="flex items-center gap-2">
                        <svg class="w-5 h-5 text-green-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                        </svg>
                        <span class="font-medium text-green-700 dark:text-green-300">Connected to O'Reilly</span>
                        <button
                          class="ml-auto text-xs px-2 py-1 rounded bg-red-100 dark:bg-red-900/20 text-red-600 dark:text-red-400 hover:bg-red-200 dark:hover:bg-red-900/40 transition-colors"
                          onClick={async () => {
                            try {
                              await invoke('oreilly_logout');
                            } catch (_) { /* ignore */ }
                            setOreillyConnected(false);
                            setOreillyStatus('Logged out. You can sign in with a different account.');
                            setOreillyEmail('');
                            setOreillyPassword('');
                          }}
                        >
                          Logout / Switch Account
                        </button>
                      </div>
                    </div>
                    <p class="text-sm text-gray-500 mb-3">
                      Search and download books from O'Reilly Learning. Books are saved to{' '}
                      <code class="px-1 py-0.5 bg-gray-200 dark:bg-gray-700 rounded text-xs">~/minion/books/oreilly/</code>
                    </p>
                    <div class="flex gap-2 mb-4">
                      <input
                        type="text"
                        class="input flex-1"
                        placeholder="Search O'Reilly books..."
                      />
                      <button class="btn btn-primary">Search</button>
                    </div>
                    <button
                      class="btn w-full flex items-center justify-center gap-2 p-3 bg-red-600 hover:bg-red-700 text-white rounded-xl transition-colors font-medium mb-2"
                      onClick={async () => {
                        try {
                          await invoke('oreilly_open_browser');
                        } catch (e) {
                          setOreillyStatus(`${e}`);
                        }
                      }}
                    >
                      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
                      </svg>
                      Browse O'Reilly Library
                    </button>
                  </Show>

                  <div class="mt-4 p-3 bg-gray-50 dark:bg-gray-800/50 rounded-lg">
                    <p class="text-xs text-gray-500 dark:text-gray-400">
                      <strong>How it works:</strong> "Use Chrome Session" reads your existing O'Reilly login
                      from Chrome (works with ACM SSO, institutional logins, etc). "Open O'Reilly Library" opens
                      an embedded browser where you can browse and read O'Reilly content directly in MINION.
                    </p>
                  </div>
                </div>

                {/* Downloaded books placeholder */}
                <Show when={true}>
                  <div class="card p-6 mt-4">
                    <h4 class="font-medium mb-3">Downloaded Books</h4>
                    <div class="text-center py-8">
                      <svg
                        class="w-10 h-10 mx-auto mb-2 text-gray-300 dark:text-gray-600"
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          stroke-width="1.5"
                          d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M9 19l3 3m0 0l3-3m-3 3V10"
                        />
                      </svg>
                      <p class="text-sm text-gray-500 dark:text-gray-400">
                        No downloaded books yet. Connect your account to get started.
                      </p>
                    </div>
                  </div>
                </Show>
              </div>
            </Show>
          </div>
        </Show>

        {/* ================================================================ */}
        {/* READER VIEW                                                      */}
        {/* ================================================================ */}
        <Show when={view() === 'reader'}>
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
                background:
                  readingMode() === 'sepia'
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
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M15 19l-7-7 7-7"
                    />
                  </svg>
                </button>
                <div>
                  <h2 class="font-medium truncate max-w-md">
                    {currentBook()?.metadata.title || loadingBookMeta()?.title || 'Loading...'}
                  </h2>
                  <p class="text-xs" style={{ color: modeStyles().mutedText }}>
                    {currentBook()
                      ? <>
                          {currentBook()!.metadata.authors?.length > 0
                            ? currentBook()!.metadata.authors.join(', ') + ' \u00B7 '
                            : ''}
                          Chapter {currentChapter() + 1} of {currentBook()!.chapters.length}
                        </>
                      : loadingBookMeta()?.author || 'Loading book content...'}
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
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M4 6h16M4 12h16M4 18h7"
                    />
                  </svg>
                </button>
              </div>
            </div>

            {/* Content area */}
            <div class="flex-1 flex overflow-hidden" style={{ 'min-height': '0' }}>
              {/* TOC Sidebar */}
              <Show when={showToc() && currentBook()}>
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
                              background:
                                index() === currentChapter()
                                  ? modeStyles().activeBg
                                  : 'transparent',
                              color:
                                index() === currentChapter()
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
                  <Show when={currentBook()} fallback={
                    /* Loading skeleton while book content loads */
                    <div class="animate-pulse space-y-6 py-4">
                      <div class="h-8 rounded-md w-3/5" style={{ background: modeStyles().hoverBg }} />
                      <div class="space-y-3">
                        <div class="h-4 rounded w-full" style={{ background: modeStyles().hoverBg }} />
                        <div class="h-4 rounded w-11/12" style={{ background: modeStyles().hoverBg }} />
                        <div class="h-4 rounded w-4/5" style={{ background: modeStyles().hoverBg }} />
                        <div class="h-4 rounded w-full" style={{ background: modeStyles().hoverBg }} />
                        <div class="h-4 rounded w-3/4" style={{ background: modeStyles().hoverBg }} />
                        <div class="h-4 rounded w-5/6" style={{ background: modeStyles().hoverBg }} />
                      </div>
                      <div class="space-y-3 pt-2">
                        <div class="h-4 rounded w-full" style={{ background: modeStyles().hoverBg }} />
                        <div class="h-4 rounded w-10/12" style={{ background: modeStyles().hoverBg }} />
                        <div class="h-4 rounded w-4/5" style={{ background: modeStyles().hoverBg }} />
                        <div class="h-4 rounded w-2/3" style={{ background: modeStyles().hoverBg }} />
                      </div>
                      <p class="text-sm text-center pt-4" style={{ color: modeStyles().mutedText }}>
                        Loading book content...
                      </p>
                    </div>
                  }>
                    <Show when={false}>
                      {/* PDF embed placeholder - convertFileSrc kept for future use */}
                      <iframe src={convertFileSrc('')} title="pdf" />
                    </Show>
                    <Show
                      when={
                        currentBook()!.format !== 'pdf' &&
                        currentBook()!.chapters[currentChapter()]
                      }
                    >
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
                  </Show>
                </div>
              </div>
            </div>

            {/* Navigation footer */}
            <Show when={currentBook()}>
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
                  style={
                    {
                      background: modeStyles().hoverBg,
                      color: modeStyles().text,
                      '--nav-hover-x': '-3px',
                      opacity: currentChapter() === 0 ? '0.35' : '1',
                      cursor: currentChapter() === 0 ? 'not-allowed' : 'pointer',
                    } as any
                  }
                  onClick={prevChapter}
                  disabled={currentChapter() === 0 || pageTransitioning()}
                >
                  <span class="flex items-center gap-1.5">
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M15 19l-7-7 7-7"
                      />
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
                  style={
                    {
                      background: modeStyles().hoverBg,
                      color: modeStyles().text,
                      '--nav-hover-x': '3px',
                      opacity:
                        currentChapter() >= currentBook()!.chapters.length - 1 ? '0.35' : '1',
                      cursor:
                        currentChapter() >= currentBook()!.chapters.length - 1
                          ? 'not-allowed'
                          : 'pointer',
                    } as any
                  }
                  onClick={nextChapter}
                  disabled={
                    currentChapter() >= currentBook()!.chapters.length - 1 || pageTransitioning()
                  }
                >
                  <span class="flex items-center gap-1.5">
                    Next
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M9 5l7 7-7 7"
                      />
                    </svg>
                  </span>
                </button>
              </div>
            </Show>
          </div>
        </Show>
      </div>
    </>
  );
};

export default Reader;
