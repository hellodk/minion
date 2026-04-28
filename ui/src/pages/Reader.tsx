import { Component, createSignal, createEffect, on, onMount, onCleanup, For, Show } from 'solid-js';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { open } from '@tauri-apps/plugin-dialog';
import { PageFlip } from 'page-flip';

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
  format: string; // "epub", "pdf", "txt", "md", "html"
  cover_base64?: string;
}

// Chapter content loaded on demand (for EPUBs)
interface ChapterContent {
  index: number;
  title: string;
  content: string; // HTML
}

type ReadingMode = 'light' | 'dark' | 'sepia';
type LibraryTab = 'all' | 'collections' | 'oreilly';
type PdfFitMode = 'fitWidth' | 'fitPage' | 'actual';

/** StPageFlip `flippingTime` (ms) — must match completion timer in EpubStPageFlip. */
const EPUB_PAGE_FLIP_MS = 960;

const PRESET_COLORS = [
  '#0ea5e9', '#8b5cf6', '#ec4899', '#f97316', '#22c55e',
  '#ef4444', '#14b8a6', '#f59e0b', '#6366f1', '#64748b',
];

// ============================================================================
// EPUB: StPageFlip — soft page curl (Apple Books–style mesh + shadows)
// ============================================================================

function coverUrl(path: string | undefined): string | undefined {
  if (!path) return undefined;
  if (path.startsWith('data:') || path.startsWith('http')) return path;
  return convertFileSrc(path);
}

async function generatePdfThumbnail(filePath: string, bookId: string): Promise<void> {
  try {
    const pdfjsLib = await import('pdfjs-dist');
    pdfjsLib.GlobalWorkerOptions.workerSrc = new URL(
      'pdfjs-dist/build/pdf.worker.mjs',
      import.meta.url,
    ).toString();

    const bytes = await invoke<number[]>('reader_get_pdf_bytes', { path: filePath });
    const data = new Uint8Array(bytes);
    const pdf = await pdfjsLib.getDocument({ data }).promise;
    const page = await pdf.getPage(1);

    const scale = 0.5;
    const viewport = page.getViewport({ scale });

    const canvas = document.createElement('canvas');
    canvas.width = Math.floor(viewport.width);
    canvas.height = Math.floor(viewport.height);
    const ctx = canvas.getContext('2d')!;

    await page.render({ canvasContext: ctx as any, canvas, viewport }).promise;

    const blob = await new Promise<Blob>((res, rej) =>
      canvas.toBlob((b) => (b ? res(b) : rej(new Error('toBlob failed'))), 'image/jpeg', 0.82)
    );
    const arrayBuffer = await blob.arrayBuffer();
    const jpegBytes = Array.from(new Uint8Array(arrayBuffer));

    await invoke('reader_save_cover', { bookId, jpegBytes });
    await pdf.destroy();
  } catch (e) {
    console.warn('PDF thumbnail generation failed:', e);
  }
}

function escapeHtmlTitle(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

interface EpubStPageFlipProps {
  dir: 'forward' | 'back';
  outgoingTitle: string;
  incomingTitle: string;
  outgoingHtml: string;
  incomingHtml: string;
  proseClass: string;
  chapterTitleColor: string;
  onComplete: () => void;
}

const EpubStPageFlip: Component<EpubStPageFlipProps> = (props) => {
  let host: HTMLDivElement | undefined;
  let pf: PageFlip | undefined;
  let doneTimer: ReturnType<typeof setTimeout> | undefined;
  let completed = false;

  const finish = () => {
    if (completed) return;
    completed = true;
    if (doneTimer !== undefined) clearTimeout(doneTimer);
    props.onComplete();
  };

  onMount(() => {
    if (!host) {
      finish();
      return;
    }

    const p0 = document.createElement('div');
    p0.dataset.density = 'soft';
    const p1 = document.createElement('div');
    p1.dataset.density = 'soft';

    const hStyle = `color: ${props.chapterTitleColor}; letter-spacing: -0.02em;`;
    const wrap = (title: string, html: string) =>
      `<h1 class="text-2xl font-bold mb-8" style="${hStyle}">${escapeHtmlTitle(title)}</h1><div class="prose max-w-none ${props.proseClass}">${html}</div>`;

    if (props.dir === 'forward') {
      p0.innerHTML = wrap(props.outgoingTitle, props.outgoingHtml);
      p1.innerHTML = wrap(props.incomingTitle, props.incomingHtml);
    } else {
      p0.innerHTML = wrap(props.incomingTitle, props.incomingHtml);
      p1.innerHTML = wrap(props.outgoingTitle, props.outgoingHtml);
    }

    try {
      pf = new PageFlip(host, {
        width: 520,
        height: 720,
        size: 'stretch',
        minWidth: 280,
        maxWidth: 960,
        minHeight: 420,
        maxHeight: 2400,
        flippingTime: EPUB_PAGE_FLIP_MS,
        usePortrait: true,
        maxShadowOpacity: 0.48,
        drawShadow: true,
        showPageCorners: false,
        useMouseEvents: false,
        disableFlipByClick: true,
        mobileScrollSupport: false,
        autoSize: true,
        showCover: false,
        startPage: props.dir === 'forward' ? 0 : 1,
        startZIndex: 0,
      });

      pf.loadFromHTML([p0, p1]);

      requestAnimationFrame(() => {
        if (!pf) return;
        if (props.dir === 'forward') {
          pf.flipNext('top');
        } else {
          pf.flipPrev('top');
        }
      });

      doneTimer = setTimeout(finish, EPUB_PAGE_FLIP_MS + 120);
    } catch (e) {
      console.error('StPageFlip failed:', e);
      finish();
    }
  });

  onCleanup(() => {
    if (doneTimer !== undefined) clearTimeout(doneTimer);
    if (pf) {
      try {
        pf.destroy();
      } catch {
        /* host may already be detached */
      }
      pf = undefined;
    }
  });

  return (
    <div
      ref={(el) => {
        host = el;
      }}
      class="epub-st-page-flip-host w-full min-h-[65vh] min-w-0"
    />
  );
};

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
  const [isFullscreen, setIsFullscreen] = createSignal(false);

  // CSS-based fullscreen: adds a class to <body> that hides the app sidebar
  // and expands the reader to fill the entire window. Esc exits.
  const enterFullscreen = () => {
    document.body.classList.add('minion-reader-fullscreen');
    setIsFullscreen(true);
  };

  const exitFullscreen = () => {
    document.body.classList.remove('minion-reader-fullscreen');
    setIsFullscreen(false);
  };

  const toggleFullscreen = () => {
    if (isFullscreen()) {
      exitFullscreen();
    } else {
      enterFullscreen();
    }
  };

  // No native fullscreenchange - we use our own CSS approach.
  // But keep the handler so onCleanup doesn't error out.
  const onFullscreenChange = () => {};

  // Animation signals
  const [pageDirection, setPageDirection] = createSignal<'left' | 'right'>('left');
  const [pageTransitioning, setPageTransitioning] = createSignal(false);
  const [bookOpening, setBookOpening] = createSignal(false);
  const [bookClosing, setBookClosing] = createSignal(false);
  const [openingCardIndex, setOpeningCardIndex] = createSignal<number | null>(null);

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

  // "Add existing books to collection" picker
  const [addBooksCollectionId, setAddBooksCollectionId] = createSignal<string | null>(null);
  const [addBooksSelected, setAddBooksSelected] = createSignal<Set<string>>(new Set<string>());
  const [addBooksFilter, setAddBooksFilter] = createSignal('');

  // Folder import modal (checkbox-based selection)
  interface FolderFileCandidate {
    path: string;
    name: string;
    extension: string;
    size: number;
    already_imported: boolean;
  }
  const [showImportModal, setShowImportModal] = createSignal(false);
  const [importModalPath, setImportModalPath] = createSignal('');
  const [importCandidates, setImportCandidates] = createSignal<FolderFileCandidate[]>([]);
  const [importSelected, setImportSelected] = createSignal<Set<string>>(new Set<string>());
  const [importLoading, setImportLoading] = createSignal(false);
  const [importFilter, setImportFilter] = createSignal('');
  const [importTargetCollection, setImportTargetCollection] = createSignal<string>('');
  const [importing, setImporting] = createSignal(false);

  // O'Reilly state
  const [oreillyEmail, setOreillyEmail] = createSignal('');
  const [oreillyPassword, setOreillyPassword] = createSignal('');
  const [oreillyConnected, setOreillyConnected] = createSignal(false);
  const [oreillyConnecting, setOreillyConnecting] = createSignal(false);
  const [oreillyStatus, setOreillyStatus] = createSignal('');

  // Loading metadata (shown while full content loads)
  const [loadingBookMeta, setLoadingBookMeta] = createSignal<{ title: string; author: string } | null>(null);

  // ---- EPUB lazy chapter loading ----
  let chapterCacheMap: Map<number, string> = new Map();
  const [chapterLoading, setChapterLoading] = createSignal(false);
  const [chapterHtml, setChapterHtml] = createSignal('');

  /** EPUB: 3D page-turn — outgoing leaf over incoming page (both full HTML snapshots). */
  const [epubTurnOutgoingHtml, setEpubTurnOutgoingHtml] = createSignal('');
  const [epubTurnIncomingHtml, setEpubTurnIncomingHtml] = createSignal('');
  const [epubTurnDir, setEpubTurnDir] = createSignal<'forward' | 'back'>('forward');
  /** While turning, `currentChapter` is still the outgoing page; this is the incoming index. */
  const [epubTurnTargetIndex, setEpubTurnTargetIndex] = createSignal<number | null>(null);

  // ---- PDF state (pdf.js) ----
  const [pdfDoc, setPdfDoc] = createSignal<any>(null);
  const [pdfCurrentPage, setPdfCurrentPage] = createSignal(1);
  const [pdfTotalPages, setPdfTotalPages] = createSignal(0);
  const [pdfZoom, setPdfZoom] = createSignal(1.5);
  /** When not `actual`, zoom is derived from container vs page size. */
  const [pdfFitMode, setPdfFitMode] = createSignal<PdfFitMode>('fitWidth');
  const [pdfLayoutTick, setPdfLayoutTick] = createSignal(0);
  const [pdfLoading, setPdfLoading] = createSignal(false);
  const [pdfPageInputValue, setPdfPageInputValue] = createSignal('1');
  let pdfCanvasRef: HTMLCanvasElement | undefined;
  let pdfContainerRef: HTMLDivElement | undefined;
  let pdfResizeObserver: ResizeObserver | undefined;
  /** Parallel EPUB loads (prefetch + single chapter) share one loading indicator. */
  let chapterLoadDepth = 0;
  /** EPUB / text reader scroll root (for scroll-to-top + parallax). */
  let textScrollContainerRef: HTMLDivElement | undefined;

  // Apple Books–style scroll-linked depth (Option C)
  let readScrollRafId = 0;
  let pendingReadScrollTarget: HTMLElement | null = null;
  let readScrollIdleTimer: ReturnType<typeof setTimeout> | undefined;

  const readingReducedMotion = () =>
    typeof window !== 'undefined' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  const flushReadScrollParallax = () => {
    readScrollRafId = 0;
    const el = pendingReadScrollTarget;
    pendingReadScrollTarget = null;
    if (!el || readingReducedMotion()) return;
    const layer = el.querySelector('.apple-books-read-layer') as HTMLElement | null;
    if (!layer) return;
    const max = Math.max(1, el.scrollHeight - el.clientHeight);
    const t = max <= 1 ? 0 : el.scrollTop / max;
    const parallax = (t - 0.5) * 10;
    layer.style.setProperty('--read-parallax-y', `${-parallax}px`);
    layer.style.setProperty('--read-shadow-alpha', String(0.05 + Math.abs(t - 0.5) * 0.1));
    layer.classList.add('read-layer-scrolling');
    if (readScrollIdleTimer) clearTimeout(readScrollIdleTimer);
    readScrollIdleTimer = setTimeout(() => {
      layer.classList.remove('read-layer-scrolling');
    }, 120);
  };

  const handleReadScroll = (e: Event) => {
    if (readingReducedMotion()) return;
    pendingReadScrollTarget = e.currentTarget as HTMLElement;
    if (readScrollRafId) return;
    readScrollRafId = requestAnimationFrame(flushReadScrollParallax);
  };

  /** Scroll EPUB/text reading pane to top (PDF uses `pdfContainerRef`). */
  const scrollTextReadingToTop = () => {
    if (textScrollContainerRef) textScrollContainerRef.scrollTop = 0;
  };

  // PDF page turn animation (prev/next / keyboard)
  const [pdfNavBusy, setPdfNavBusy] = createSignal(false);
  const [pdfPageSwapClass, setPdfPageSwapClass] = createSignal('');

  const runPdfPageChange = async (targetPage: number, dir: 'forward' | 'back') => {
    if (pdfNavBusy()) return;
    const doc = pdfDoc();
    if (!doc || !pdfCanvasRef) return;
    const n = Math.max(1, Math.min(Math.floor(targetPage), doc.numPages));
    if (n === pdfCurrentPage()) return;

    setPdfNavBusy(true);
    try {
      if (readingReducedMotion()) {
        await renderPdfPage(n);
        savePdfProgress(n);
        if (pdfContainerRef) pdfContainerRef.scrollTop = 0;
        return;
      }
      const outCls = dir === 'forward' ? 'pdf-page-out-left' : 'pdf-page-out-right';
      const inCls = dir === 'forward' ? 'pdf-page-in-right' : 'pdf-page-in-left';
      setPdfPageSwapClass(outCls);
      await new Promise<void>((r) => setTimeout(r, 200));
      await renderPdfPage(n);
      setPdfPageSwapClass(inCls);
      await new Promise<void>((r) => setTimeout(r, 260));
      setPdfPageSwapClass('');
      savePdfProgress(n);
      if (pdfContainerRef) pdfContainerRef.scrollTop = 0;
    } finally {
      setPdfNavBusy(false);
    }
  };

  // ============================================================================
  // Keyboard navigation
  // ============================================================================

  const handleKeyDown = (e: KeyboardEvent) => {
    if (view() !== 'reader' || !currentBook()) return;
    if (pageTransitioning()) return;

    const book = currentBook()!;
    const isPdf = book.format === 'pdf';
    if (isPdf && pdfNavBusy()) return;

    if (e.key === 'ArrowRight') {
      e.preventDefault();
      if (isPdf) {
        nextPdfPage();
      } else {
        nextChapter();
      }
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault();
      if (isPdf) {
        prevPdfPage();
      } else {
        prevChapter();
      }
    } else if (e.key === 'Escape') {
      e.preventDefault();
      if (isFullscreen()) {
        exitFullscreen();
      } else {
        closeBook();
      }
    } else if (e.key === 'F11' || (e.key === 'f' && !e.ctrlKey && !e.metaKey && !e.altKey)) {
      // F11 or 'f' key toggles fullscreen (like video players)
      e.preventDefault();
      toggleFullscreen();
    }
  };

  // ============================================================================
  // Lifecycle
  // ============================================================================

  onMount(async () => {
    document.addEventListener('keydown', handleKeyDown);
    document.addEventListener('fullscreenchange', onFullscreenChange);
    await Promise.all([loadLibrary(), loadCollections()]);
  });

  onCleanup(() => {
    document.removeEventListener('keydown', handleKeyDown);
    document.removeEventListener('fullscreenchange', onFullscreenChange);
    // Make sure fullscreen class is removed when leaving the Reader page
    document.body.classList.remove('minion-reader-fullscreen');
    if (readScrollIdleTimer) clearTimeout(readScrollIdleTimer);
    pdfResizeObserver?.disconnect();
    pdfResizeObserver = undefined;
    // Clean up pdf.js document
    const doc = pdfDoc();
    if (doc) {
      doc.destroy().catch(() => {});
    }
    // Clean up temp EPUB image dir for the open book
    const openBook = currentBook();
    if (openBook?.file_path && openBook.format === 'epub') {
      void invoke('reader_cleanup_book_images', { bookPath: openBook.file_path });
    }
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
  // EPUB lazy chapter loading
  // ============================================================================

  const fetchChapterIntoCache = async (index: number): Promise<string> => {
    const book = currentBook();
    if (!book?.file_path) return '<p>No content available.</p>';

    const chapter = await invoke<ChapterContent>('reader_get_chapter', {
      path: book.file_path,
      chapterIndex: index,
    });
    chapterCacheMap.set(index, chapter.content);
    return chapter.content;
  };

  /** One `EpubDoc` open on the backend for several chapters — faster than repeated `reader_get_chapter`. */
  const prefetchEpubChapters = async (indices: number[]) => {
    const book = currentBook();
    if (!book?.file_path || book.format !== 'epub') return;

    const missing = [...new Set(indices)].filter(
      i => i >= 0 && i < book.chapters.length && !chapterCacheMap.has(i)
    );
    if (missing.length === 0) return;

    chapterLoadDepth++;
    if (chapterLoadDepth === 1) setChapterLoading(true);
    try {
      try {
        const chapters = await invoke<ChapterContent[]>('reader_prefetch_epub_chapters', {
          path: book.file_path,
          indices: missing,
        });
        for (const ch of chapters) {
          chapterCacheMap.set(ch.index, ch.content);
        }
      } catch (e) {
        console.warn('Batch EPUB prefetch failed, loading sequentially:', e);
        for (const i of missing) {
          try {
            await fetchChapterIntoCache(i);
          } catch (err) {
            console.error(`Chapter ${i}:`, err);
            chapterCacheMap.set(
              i,
              `<p style="color:red;">Failed to load chapter: ${err}</p>`
            );
          }
        }
      }
    } finally {
      chapterLoadDepth--;
      if (chapterLoadDepth === 0) setChapterLoading(false);
    }
  };

  const chapterTitleForIndex = (idx: number) => {
    const book = currentBook();
    if (!book || idx < 0 || idx >= book.chapters.length) {
      return `Chapter ${idx + 1}`;
    }
    return book.chapters[idx].title || `Chapter ${idx + 1}`;
  };

  const completeEpubChapterTurn = (targetIndex: number, incoming: string) => {
    setCurrentChapter(targetIndex);
    setChapterHtml(incoming);
    setEpubTurnOutgoingHtml('');
    setEpubTurnIncomingHtml('');
    setEpubTurnTargetIndex(null);
    setPageTransitioning(false);
    scrollTextReadingToTop();
    saveProgress(targetIndex);
    const book = currentBook();
    if (book && book.format === 'epub') {
      void prefetchEpubChapters(
        [targetIndex - 1, targetIndex, targetIndex + 1, targetIndex + 2].filter(
          i => i >= 0 && i < book.chapters.length
        )
      );
    }
  };

  /**
   * EPUB chapter change: StPageFlip soft curl (mesh + shadows), like Apple Books.
   */
  const runEpubPageTurn = async (targetIndex: number, dir: 'forward' | 'back') => {
    const book = currentBook();
    if (!book || book.format !== 'epub' || pageTransitioning()) return;
    if (targetIndex < 0 || targetIndex >= book.chapters.length) return;
    if (targetIndex === currentChapter()) return;
    if (!chapterHtml()) return;

    if (readingReducedMotion()) {
      setPageTransitioning(true);
      setPageDirection(dir === 'forward' ? 'left' : 'right');
      await prefetchEpubChapters(
        [targetIndex, targetIndex + 1, targetIndex - 1, targetIndex + 2].filter(
          i => i >= 0 && i < book.chapters.length
        )
      );
      let html = chapterCacheMap.get(targetIndex);
      if (!html) {
        try {
          html = await fetchChapterIntoCache(targetIndex);
        } catch (e) {
          console.error(e);
          setPageTransitioning(false);
          return;
        }
      }
      setCurrentChapter(targetIndex);
      setChapterHtml(html);
      scrollTextReadingToTop();
      saveProgress(targetIndex);
      setPageTransitioning(false);
      return;
    }

    setPageDirection(dir === 'forward' ? 'left' : 'right');
    setPageTransitioning(true);

    await prefetchEpubChapters(
      [targetIndex, targetIndex + 1, targetIndex - 1, targetIndex + 2].filter(
        i => i >= 0 && i < book.chapters.length
      )
    );

    let incoming = chapterCacheMap.get(targetIndex);
    if (!incoming) {
      try {
        incoming = await fetchChapterIntoCache(targetIndex);
      } catch (e) {
        console.error('Failed to load chapter for page turn:', e);
        setPageTransitioning(false);
        return;
      }
    }

    const outgoing = chapterHtml();
    setEpubTurnDir(dir);
    setEpubTurnTargetIndex(targetIndex);
    setEpubTurnOutgoingHtml(outgoing);
    setEpubTurnIncomingHtml(incoming);

    scrollTextReadingToTop();
  };

  // When the current chapter changes for an EPUB, prefetch current + neighbors in one batch
  createEffect(on(
    () => [currentChapter(), currentBook()?.format] as const,
    async ([chapIdx, format]) => {
      if (format !== 'epub' || !currentBook()) return;
      const book = currentBook()!;
      const max = book.chapters.length;
      const indices = [chapIdx - 1, chapIdx, chapIdx + 1, chapIdx + 2].filter(
        i => i >= 0 && i < max
      );
      await prefetchEpubChapters(indices);
      setChapterHtml(chapterCacheMap.get(chapIdx) || '');
    },
    { defer: true }
  ));

  // ============================================================================
  // PDF rendering with pdf.js
  // ============================================================================

  const normalizePdfBytes = (data: unknown): Uint8Array => {
    if (data instanceof Uint8Array) return data;
    if (data instanceof ArrayBuffer) return new Uint8Array(data);
    if (Array.isArray(data)) return new Uint8Array(data);
    return new Uint8Array();
  };

  const loadPdf = async (filePath: string) => {
    setPdfLoading(true);
    try {
      // Dynamically import pdfjs-dist
      const pdfjsLib = await import('pdfjs-dist');

      // Set worker source
      pdfjsLib.GlobalWorkerOptions.workerSrc = new URL(
        'pdfjs-dist/build/pdf.worker.mjs',
        import.meta.url
      ).toString();

      // Read PDF bytes through Tauri IPC (serde may deliver number[] or Uint8Array)
      const pdfRaw = await invoke<unknown>('reader_get_pdf_bytes', { path: filePath });
      const pdfBytes = normalizePdfBytes(pdfRaw);
      if (pdfBytes.length === 0) {
        throw new Error('PDF file is empty or could not be read');
      }
      const doc = await pdfjsLib.getDocument({ data: pdfBytes }).promise;

      setPdfDoc(doc);
      setPdfTotalPages(doc.numPages);

      // Restore page position
      const imported = currentBookId();
      if (imported) {
        const lib = libraryBooks().find(b => b.id === imported);
        if (lib?.current_position) {
          const savedPage = parseInt(lib.current_position, 10);
          if (savedPage > 0 && savedPage <= doc.numPages) {
            setPdfCurrentPage(savedPage);
            setPdfPageInputValue(String(savedPage));
          }
        }
      }
    } catch (e) {
      console.error('Failed to load PDF:', e);
      alert(`Error loading PDF: ${e}`);
    } finally {
      setPdfLoading(false);
    }

    // First paint happens in the <canvas> ref callback once Solid mounts the canvas
    // (`Show when={!pdfLoading() && pdfDoc()}`). requestAnimationFrame retries can run
    // before refs run, so the canvas stayed blank.
  };

  const getPdfRenderScale = async (page: { getViewport: (o: { scale: number }) => { width: number; height: number } }) => {
    const mode = pdfFitMode();
    if (mode === 'actual') return pdfZoom();
    if (!pdfContainerRef) return pdfZoom();
    const base = page.getViewport({ scale: 1 });
    const padX = 40;
    const cw = Math.max(80, pdfContainerRef.clientWidth - padX);
    const ch = Math.max(80, pdfContainerRef.clientHeight - 48);
    if (mode === 'fitWidth') {
      return cw / base.width;
    }
    return Math.min(cw / base.width, ch / base.height);
  };

  const renderPdfPage = async (pageNum: number) => {
    const doc = pdfDoc();
    if (!doc || !pdfCanvasRef) return;

    try {
      const n = Math.max(1, Math.min(Math.floor(Number(pageNum)) || 1, doc.numPages));
      const page = await doc.getPage(n);
      const scale = await getPdfRenderScale(page);
      const viewport = page.getViewport({ scale });

      // Set canvas size accounting for device pixel ratio for crisp rendering
      const dpr = window.devicePixelRatio || 1;
      pdfCanvasRef.width = Math.floor(viewport.width * dpr);
      pdfCanvasRef.height = Math.floor(viewport.height * dpr);
      pdfCanvasRef.style.width = `${Math.floor(viewport.width)}px`;
      pdfCanvasRef.style.height = `${Math.floor(viewport.height)}px`;

      const ctx = pdfCanvasRef.getContext('2d')!;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

      await page.render({ canvasContext: ctx, viewport }).promise;

      setPdfCurrentPage(n);
      setPdfPageInputValue(String(n));
    } catch (e) {
      console.error('Failed to render PDF page:', e);
    }
  };

  // Re-render PDF when zoom, fit mode, or container size changes
  createEffect(on(
    () => [pdfDoc(), pdfFitMode(), pdfZoom(), pdfLayoutTick()] as const,
    () => {
      if (pdfDoc()) {
        void renderPdfPage(pdfCurrentPage());
      }
    },
    { defer: true }
  ));

  const nextPdfPage = () => {
    if (pdfCurrentPage() >= pdfTotalPages()) return;
    void runPdfPageChange(pdfCurrentPage() + 1, 'forward');
  };

  const prevPdfPage = () => {
    if (pdfCurrentPage() <= 1) return;
    void runPdfPageChange(pdfCurrentPage() - 1, 'back');
  };

  const goToPdfPage = (page: number) => {
    if (page < 1 || page > pdfTotalPages()) return;
    const cur = pdfCurrentPage();
    if (page === cur) return;
    void runPdfPageChange(page, page > cur ? 'forward' : 'back');
  };

  const handlePdfPageInput = (e: KeyboardEvent) => {
    if (e.key === 'Enter') {
      const val = parseInt(pdfPageInputValue(), 10);
      if (!isNaN(val)) goToPdfPage(val);
    }
  };

  const savePdfProgress = async (page: number) => {
    const bookId = currentBookId();
    if (!bookId) return;
    const total = pdfTotalPages();
    const progress = total > 0 ? (page / total) * 100 : 0;
    try {
      await invoke('reader_update_progress', {
        bookId,
        progress,
        position: String(page),
      });
    } catch (e) {
      console.error('Failed to save PDF progress:', e);
    }
  };

  const toggleReaderFullscreen = async () => {
    try {
      const win = getCurrentWindow();
      await win.setFullscreen(!(await win.isFullscreen()));
    } catch {
      try {
        if (!document.fullscreenElement) {
          await document.documentElement.requestFullscreen();
        } else {
          await document.exitFullscreen();
        }
      } catch (e) {
        console.warn('Fullscreen not available:', e);
      }
    }
  };

  const pdfZoomLabel = () => {
    const mode = pdfFitMode();
    if (mode === 'fitWidth') return 'Fit width';
    if (mode === 'fitPage') return 'Fit page';
    return `${Math.round(pdfZoom() * 100)}%`;
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

  // Open folder picker, then show a modal with checkbox list of files to import
  const browseForFolderWithSelection = async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (selected && typeof selected === 'string') {
        setImportModalPath(selected);
        setShowImportModal(true);
        setImportLoading(true);
        setImportCandidates([]);
        setImportSelected(new Set<string>());
        setImportFilter('');
        setImportTargetCollection('');
        try {
          const files = await invoke<FolderFileCandidate[]>('reader_list_folder_files', {
            path: selected,
          });
          setImportCandidates(files);
          // Pre-select all files that aren't already imported
          const preSelected = new Set(
            files.filter((f) => !f.already_imported).map((f) => f.path)
          );
          setImportSelected(preSelected);
        } catch (e) {
          console.error('Failed to list folder files:', e);
          alert(`Failed to scan folder: ${e}`);
          setShowImportModal(false);
        } finally {
          setImportLoading(false);
        }
      }
    } catch (e) {
      console.error('Failed to open folder dialog:', e);
      alert(`Error: ${e}`);
    }
  };

  // Also let user add files (multi-select) from file picker
  const browseForMultipleFiles = async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [
          {
            name: 'Books',
            extensions: ['epub', 'pdf', 'mobi', 'azw3', 'fb2', 'txt', 'md', 'markdown', 'html', 'htm'],
          },
        ],
      });
      if (selected && Array.isArray(selected) && selected.length > 0) {
        const paths = selected as string[];
        setImporting(true);
        try {
          const result = await invoke<{ imported: number; skipped: number; failed: number }>(
            'reader_import_paths',
            { paths, collectionId: null }
          );
          await loadLibrary();
          // Generate thumbnails for any newly imported PDFs that lack covers
          const pdfsNeedingThumbs = libraryBooks().filter(
            (b) => b.format === 'pdf' && !b.cover_path
          );
          for (const b of pdfsNeedingThumbs) {
            void generatePdfThumbnail(b.file_path, b.id);
          }
          alert(
            `Imported: ${result.imported}, Skipped (already exists): ${result.skipped}, Failed: ${result.failed}`
          );
        } catch (e) {
          alert(`Import failed: ${e}`);
        } finally {
          setImporting(false);
        }
      }
    } catch (e) {
      console.error('Failed to open file dialog:', e);
    }
  };

  const toggleImportSelection = (path: string) => {
    const current = importSelected();
    const next = new Set<string>(current);
    if (next.has(path)) {
      next.delete(path);
    } else {
      next.add(path);
    }
    setImportSelected(next);
  };

  const selectAllImport = () => {
    const newSet = new Set<string>(
      importCandidates().filter((c) => !c.already_imported).map((c) => c.path)
    );
    setImportSelected(newSet);
  };

  const deselectAllImport = () => {
    setImportSelected(new Set<string>());
  };

  const filteredImportCandidates = () => {
    const q = importFilter().trim().toLowerCase();
    if (!q) return importCandidates();
    return importCandidates().filter(
      (c) => c.name.toLowerCase().includes(q) || c.extension.toLowerCase().includes(q)
    );
  };

  const confirmImportSelection = async () => {
    const paths = Array.from(importSelected());
    if (paths.length === 0) {
      alert('No files selected');
      return;
    }
    setImporting(true);
    try {
      const result = await invoke<{ imported: number; skipped: number; failed: number }>(
        'reader_import_paths',
        {
          paths,
          collectionId: importTargetCollection() || null,
        }
      );
      await loadLibrary();
      await loadCollections();
      // Generate thumbnails for any newly imported PDFs that lack covers
      const pdfsNeedingThumbs = libraryBooks().filter(
        (b) => b.format === 'pdf' && !b.cover_path
      );
      for (const b of pdfsNeedingThumbs) {
        void generatePdfThumbnail(b.file_path, b.id);
      }
      setShowImportModal(false);
      alert(
        `Imported: ${result.imported}\nSkipped (already exists): ${result.skipped}\nFailed: ${result.failed}`
      );
    } catch (e) {
      alert(`Import failed: ${e}`);
    } finally {
      setImporting(false);
    }
  };

  const formatBytes = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    const kb = bytes / 1024;
    if (kb < 1024) return `${kb.toFixed(1)} KB`;
    const mb = kb / 1024;
    if (mb < 1024) return `${mb.toFixed(1)} MB`;
    return `${(mb / 1024).toFixed(2)} GB`;
  };

  const openBookByPath = async (path: string, cardIndex?: number) => {
    if (loading() || bookClosing()) return;
    setLoading(true);

    // Clear previous state
    chapterCacheMap = new Map();
    setChapterHtml('');
    setEpubTurnOutgoingHtml('');
    setEpubTurnIncomingHtml('');
    setEpubTurnTargetIndex(null);
    setPdfDoc(null);
    setPdfCurrentPage(1);
    setPdfTotalPages(0);
    setPdfPageInputValue('1');
    setPdfFitMode('fitWidth');
    setPdfLayoutTick(0);

    try {
      // Import book to DB for metadata
      const imported = await invoke<LibraryBook>('reader_import_book', { path });
      setCurrentBookId(imported.id);

      // Load book structure (metadata + chapter list, no content for EPUBs)
      const content = await invoke<BookContent>('reader_open_book', { path });
      setCurrentBook(content);

      if (content.format === 'epub') {
        // Restore EPUB position
        const startChapter = imported.current_position
          ? parseInt(imported.current_position, 10) || 0
          : 0;
        setCurrentChapter(Math.min(startChapter, Math.max(0, content.chapters.length - 1)));
        // The createEffect will load the chapter content
      } else if (content.format === 'pdf') {
        // PDF: position is restored inside loadPdf
        setCurrentChapter(0);
      } else {
        // TXT/MD/HTML: content is in chapters[0].content
        setCurrentChapter(0);
      }

      // Show reader with open animation
      if (cardIndex !== undefined) setOpeningCardIndex(cardIndex);
      setBookOpening(true);
      setView('reader');
      setTimeout(() => { setBookOpening(false); setOpeningCardIndex(null); }, 500);

      // For PDFs, start loading after view transition
      if (content.format === 'pdf' && content.file_path) {
        setTimeout(() => loadPdf(content.file_path!), 100);
      }

      // Generate PDF thumbnail if this book has no cover yet
      if (content.format === 'pdf' && content.file_path && !imported.cover_path) {
        void generatePdfThumbnail(content.file_path, imported.id);
      }

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
    const book = currentBook();
    if (book?.file_path && book.format === 'epub') {
      void invoke('reader_cleanup_book_images', { bookPath: book.file_path });
    }
    // Always exit fullscreen when closing book
    if (isFullscreen()) exitFullscreen();
    setBookClosing(true);
    setTimeout(() => {
      // Clean up PDF doc
      const doc = pdfDoc();
      if (doc) {
        doc.destroy().catch(() => {});
        setPdfDoc(null);
      }
      setCurrentBook(null);
      setCurrentBookId(null);
      setLoadingBookMeta(null);
      setChapterHtml('');
      setEpubTurnOutgoingHtml('');
      setEpubTurnIncomingHtml('');
      setEpubTurnTargetIndex(null);
      chapterCacheMap = new Map();
      setView('library');
      setBookClosing(false);
      setShowToc(false);
      setPdfPageSwapClass('');
      setPdfFitMode('fitWidth');
      loadLibrary();
    }, 350);
  };

  // ============================================================================
  // Chapter navigation with progress persistence (EPUB)
  // ============================================================================

  const saveProgress = async (chapterIdx: number) => {
    const bookId = currentBookId();
    const book = currentBook();
    if (!bookId || !book) return;

    const progress = book.chapters.length > 0
      ? ((chapterIdx + 1) / book.chapters.length) * 100
      : 0;
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

    const newChapter = currentChapter() + 1;
    if (isEpub()) {
      void runEpubPageTurn(newChapter, 'forward');
    } else {
      setPageDirection('left');
      setPageTransitioning(true);
      setTimeout(() => {
        setCurrentChapter(newChapter);
        scrollTextReadingToTop();
        saveProgress(newChapter);
      }, 260);
      setTimeout(() => {
        setPageTransitioning(false);
      }, 540);
    }
  };

  const prevChapter = () => {
    if (currentChapter() <= 0) return;
    if (pageTransitioning()) return;

    const newChapter = currentChapter() - 1;
    if (isEpub()) {
      void runEpubPageTurn(newChapter, 'back');
    } else {
      setPageDirection('right');
      setPageTransitioning(true);
      setTimeout(() => {
        setCurrentChapter(newChapter);
        scrollTextReadingToTop();
        saveProgress(newChapter);
      }, 260);
      setTimeout(() => {
        setPageTransitioning(false);
      }, 540);
    }
  };

  const goToChapter = (index: number) => {
    if (pageTransitioning()) return;
    const cur = currentChapter();
    if (index === cur) return;

    if (isEpub()) {
      void runEpubPageTurn(index, index > cur ? 'forward' : 'back');
      setShowToc(false);
    } else {
      setPageDirection(index > cur ? 'left' : 'right');
      setPageTransitioning(true);
      setTimeout(() => {
        setCurrentChapter(index);
        setShowToc(false);
        scrollTextReadingToTop();
        saveProgress(index);
      }, 260);
      setTimeout(() => {
        setPageTransitioning(false);
      }, 540);
    }
  };

  // ============================================================================
  // Card 3D tilt
  // ============================================================================

  const handleCardMouseMove = (e: MouseEvent) => {
    const wrapper = e.currentTarget as HTMLElement;
    const card = wrapper.querySelector('.book-card-inner') as HTMLElement | null;
    if (!card) return;
    const rect = wrapper.getBoundingClientRect();
    const x = (e.clientX - rect.left) / rect.width - 0.5;
    const y = (e.clientY - rect.top) / rect.height - 0.5;
    card.style.setProperty('--tilt-x', `${(-y * 12).toFixed(1)}deg`);
    card.style.setProperty('--tilt-y', `${(x * 12).toFixed(1)}deg`);
  };

  const handleCardMouseLeave = (e: MouseEvent) => {
    const wrapper = e.currentTarget as HTMLElement;
    const card = wrapper.querySelector('.book-card-inner') as HTMLElement | null;
    if (!card) return;
    card.style.setProperty('--tilt-x', '0deg');
    card.style.setProperty('--tilt-y', '0deg');
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

  // Open the "add existing books" picker for a collection
  const openAddBooksPicker = (collectionId: string) => {
    setAddBooksCollectionId(collectionId);
    setAddBooksSelected(new Set<string>());
    setAddBooksFilter('');
  };

  const closeAddBooksPicker = () => {
    setAddBooksCollectionId(null);
    setAddBooksSelected(new Set<string>());
    setAddBooksFilter('');
  };

  const toggleAddBookSelection = (bookId: string) => {
    const next = new Set<string>(addBooksSelected());
    if (next.has(bookId)) next.delete(bookId);
    else next.add(bookId);
    setAddBooksSelected(next);
  };

  const booksNotInCollection = () => {
    const inCollection = new Set(collectionBooks().map((b) => b.id));
    const q = addBooksFilter().trim().toLowerCase();
    return libraryBooks().filter((b) => {
      if (inCollection.has(b.id)) return false;
      if (!q) return true;
      return (
        (b.title || '').toLowerCase().includes(q) ||
        (b.authors || '').toLowerCase().includes(q) ||
        (b.file_path || '').toLowerCase().includes(q)
      );
    });
  };

  const confirmAddBooksToCollection = async () => {
    const collectionId = addBooksCollectionId();
    if (!collectionId) return;
    const ids = Array.from(addBooksSelected());
    if (ids.length === 0) {
      closeAddBooksPicker();
      return;
    }
    try {
      for (const bookId of ids) {
        await invoke('reader_add_to_collection', { collectionId, bookId });
      }
      await loadCollections();
      if (expandedCollection() === collectionId) {
        const books = await invoke<LibraryBook[]>('reader_get_collection_books', {
          collectionId,
        });
        setCollectionBooks(books);
      }
      closeAddBooksPicker();
    } catch (e) {
      console.error('Failed to add books to collection:', e);
      alert(`Failed to add books: ${e}`);
    }
  };

  // ============================================================================
  // Derived values
  // ============================================================================

  const isPdf = () => currentBook()?.format === 'pdf';
  const isEpub = () => currentBook()?.format === 'epub';

  const progressPercent = () => {
    const book = currentBook();
    if (!book) return 0;
    if (isPdf()) {
      return pdfTotalPages() > 0 ? (pdfCurrentPage() / pdfTotalPages()) * 100 : 0;
    }
    if (book.chapters.length === 0) return 0;
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
        onMouseMove={handleCardMouseMove}
        onMouseLeave={handleCardMouseLeave}
      >
        <div
          class="book-card-inner card p-3 relative overflow-hidden"
          style={{ transform: 'rotateX(var(--tilt-x)) rotateY(var(--tilt-y)) translateZ(0px)' }}
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
              {(path) => (
                <img
                  src={coverUrl(path())}
                  alt={displayTitle}
                  class="w-full h-full object-cover rounded-lg"
                  onError={(e) => { (e.target as HTMLImageElement).style.display = 'none'; }}
                />
              )}
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

        {/* Add to Collection button - always visible */}
        <div
          class="absolute top-1 right-1 z-10"
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
              class="w-7 h-7 rounded-full bg-sky-500 text-white flex items-center justify-center text-xs shadow-md hover:bg-sky-600 transition-all hover:scale-110"
              title="Add to collection"
              onClick={(e) => {
                e.stopPropagation();
                setAddToCollectionBookId(
                  addToCollectionBookId() === book.id ? null : book.id
                );
              }}
            >
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2.5"
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
          --tilt-x: 0deg;
          --tilt-y: 0deg;
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
            transform: perspective(1400px) translateX(0) translateZ(0) rotateY(0deg);
            opacity: 1;
            transform-origin: left center;
          }
          100% {
            transform: perspective(1400px) translateX(-42px) translateZ(-12px) rotateY(18deg);
            opacity: 0;
            transform-origin: left center;
          }
        }

        @keyframes pageEnterRight {
          0% {
            transform: perspective(1400px) translateX(56px) translateZ(-8px) rotateY(-12deg);
            opacity: 0;
            transform-origin: right center;
          }
          100% {
            transform: perspective(1400px) translateX(0) translateZ(0) rotateY(0deg);
            opacity: 1;
            transform-origin: right center;
          }
        }

        @keyframes pageExitRight {
          0% {
            transform: perspective(1400px) translateX(0) translateZ(0) rotateY(0deg);
            opacity: 1;
            transform-origin: right center;
          }
          100% {
            transform: perspective(1400px) translateX(42px) translateZ(-12px) rotateY(-18deg);
            opacity: 0;
            transform-origin: right center;
          }
        }

        @keyframes pageEnterLeft {
          0% {
            transform: perspective(1400px) translateX(-56px) translateZ(-8px) rotateY(12deg);
            opacity: 0;
            transform-origin: left center;
          }
          100% {
            transform: perspective(1400px) translateX(0) translateZ(0) rotateY(0deg);
            opacity: 1;
            transform-origin: left center;
          }
        }

        .page-exit-left {
          animation: pageExitLeft 0.42s cubic-bezier(0.22, 1, 0.36, 1) forwards;
        }

        .page-enter-right {
          animation: pageEnterRight 0.48s cubic-bezier(0.22, 1, 0.36, 1) forwards;
        }

        .page-exit-right {
          animation: pageExitRight 0.42s cubic-bezier(0.22, 1, 0.36, 1) forwards;
        }

        .page-enter-left {
          animation: pageEnterLeft 0.48s cubic-bezier(0.22, 1, 0.36, 1) forwards;
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

        /* Shared prose improvements (Apple Books–like rhythm) */
        .sepia-prose, .dark-prose, .light-prose {
          line-height: 1.78;
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

        /* Scroll roots (EPUB + PDF): native momentum; avoid CSS smooth scroll (hurts trackpad). */
        [data-reading-scroll] {
          scroll-behavior: auto;
          overscroll-behavior-y: contain;
          -webkit-overflow-scrolling: touch;
        }

        /* Apple Books–style scroll-linked depth (Option C) */
        .apple-books-read-layer {
          transform: translate3d(0, var(--read-parallax-y, 0px), 0);
          box-shadow:
            0 1px 0 rgba(0, 0, 0, calc(var(--read-shadow-alpha, 0.06))),
            0 16px 48px -20px rgba(0, 0, 0, calc(var(--read-shadow-alpha, 0.04)));
        }

        @media (prefers-reduced-motion: reduce) {
          .apple-books-read-layer {
            transform: none !important;
            box-shadow: none !important;
          }
        }

        .read-layer-scrolling {
          will-change: transform;
        }

        .reader-chapter-perspective {
          perspective: 1400px;
        }

        .reader-chapter-stage {
          overflow-x: hidden;
        }

        .reader-chapter-stage:has(.epub-page-turn-stage) {
          overflow: visible;
        }

        /* EPUB: StPageFlip root (soft curl — library injects .stf__* layers + shadows) */
        .epub-page-turn-stage {
          position: relative;
          overflow: visible;
        }

        .epub-page-turn-scroll {
          overflow-x: visible !important;
        }

        /* PDF page turn (prev/next) — softer slide + scale like a turning sheet */
        .pdf-page-swap {
          display: inline-block;
          transition:
            transform 0.22s cubic-bezier(0.22, 1, 0.36, 1),
            opacity 0.22s cubic-bezier(0.22, 1, 0.36, 1);
        }

        .pdf-page-swap.pdf-page-out-left {
          transform: translateX(-28px) scale(0.97) rotateY(4deg);
          opacity: 0.2;
        }

        .pdf-page-swap.pdf-page-out-right {
          transform: translateX(28px) scale(0.97) rotateY(-4deg);
          opacity: 0.2;
        }

        .pdf-page-swap.pdf-page-in-right {
          animation: pdfPageInFromRight 0.34s cubic-bezier(0.22, 1, 0.36, 1) forwards;
        }

        .pdf-page-swap.pdf-page-in-left {
          animation: pdfPageInFromLeft 0.34s cubic-bezier(0.22, 1, 0.36, 1) forwards;
        }

        @keyframes pdfPageInFromRight {
          from {
            opacity: 0;
            transform: translateX(36px) scale(0.98);
          }
          to {
            opacity: 1;
            transform: translateX(0) scale(1);
          }
        }

        @keyframes pdfPageInFromLeft {
          from {
            opacity: 0;
            transform: translateX(-36px) scale(0.98);
          }
          to {
            opacity: 1;
            transform: translateX(0) scale(1);
          }
        }

        @media (prefers-reduced-motion: reduce) {
          .pdf-page-swap {
            transition: none;
            animation: none !important;
            transform: none !important;
            opacity: 1 !important;
          }
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

        /* PDF canvas container */
        .pdf-canvas-container {
          display: flex;
          justify-content: center;
          align-items: flex-start;
          min-height: 100%;
        }

        .pdf-canvas-container canvas {
          box-shadow: 0 4px 24px rgba(0, 0, 0, 0.15);
          border-radius: 2px;
        }

        /* Chapter loading spinner */
        @keyframes chapterSpin {
          to { transform: rotate(360deg); }
        }

        .chapter-spinner {
          width: 32px;
          height: 32px;
          border: 3px solid rgba(0, 0, 0, 0.1);
          border-top-color: #0ea5e9;
          border-radius: 50%;
          animation: chapterSpin 0.8s linear infinite;
        }

        /* Zoom controls */
        .zoom-btn {
          width: 32px;
          height: 32px;
          border-radius: 6px;
          display: flex;
          align-items: center;
          justify-content: center;
          font-size: 16px;
          font-weight: 700;
          cursor: pointer;
          transition: background 0.15s, transform 0.1s;
          user-select: none;
        }

        .zoom-btn:hover {
          transform: scale(1.08);
        }

        .zoom-btn:active {
          transform: scale(0.95);
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
              <div class="flex gap-2 flex-wrap">
                <button
                  class="btn btn-secondary text-sm"
                  onClick={browseForMultipleFiles}
                  disabled={loading() || importing()}
                  title="Pick multiple book files to add to the library"
                >
                  <svg class="w-4 h-4 mr-1.5 inline-block" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 13h6m-3-3v6m-9 1V7a2 2 0 012-2h6l2 2h6a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z" />
                  </svg>
                  Add Files
                </button>
                <button
                  class="btn btn-secondary text-sm"
                  onClick={browseForFolderWithSelection}
                  disabled={loading() || importing()}
                  title="Pick a folder and choose which files to import"
                >
                  <svg class="w-4 h-4 mr-1.5 inline-block" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-3-3v6m-9 1V7a2 2 0 012-2h5l2 2h5a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z" />
                  </svg>
                  Add Folder
                </button>
                <button
                  class="btn btn-secondary text-sm"
                  onClick={browseForLibraryFolder}
                  disabled={loading()}
                  title="Scan a folder and import all books automatically"
                >
                  <svg class="w-4 h-4 mr-1.5 inline-block" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                  </svg>
                  Quick Scan
                </button>
                <button
                  class="btn btn-primary text-sm"
                  onClick={browseForBook}
                  disabled={loading()}
                  title="Open a single book file"
                >
                  <svg class="w-4 h-4 mr-1.5 inline-block" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6v6m0 0v6m0-6h6m-6 0H6" />
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
                            {/* Action bar inside collection */}
                            <div class="flex gap-2 mb-4">
                              <button
                                class="btn btn-primary text-xs"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  openAddBooksPicker(col.id);
                                }}
                                title="Add books already in your library to this collection"
                              >
                                <svg class="w-4 h-4 mr-1.5 inline-block" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
                                </svg>
                                Add Books
                              </button>
                              <button
                                class="btn btn-secondary text-xs"
                                onClick={async (e) => {
                                  e.stopPropagation();
                                  // Pre-set target collection then open the import-folder modal
                                  setImportTargetCollection(col.id);
                                  await browseForFolderWithSelection();
                                }}
                                title="Import new books from a folder into this collection"
                              >
                                <svg class="w-4 h-4 mr-1.5 inline-block" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                                </svg>
                                Import Folder
                              </button>
                              <button
                                class="btn btn-secondary text-xs"
                                onClick={async (e) => {
                                  e.stopPropagation();
                                  // File picker then import to this collection
                                  try {
                                    const selected = await open({
                                      multiple: true,
                                      filters: [
                                        {
                                          name: 'Books',
                                          extensions: ['epub', 'pdf', 'mobi', 'azw3', 'fb2', 'txt', 'md', 'markdown', 'html', 'htm'],
                                        },
                                      ],
                                    });
                                    if (selected && Array.isArray(selected) && selected.length > 0) {
                                      const paths = selected as string[];
                                      setImporting(true);
                                      try {
                                        await invoke<{ imported: number; skipped: number; failed: number }>(
                                          'reader_import_paths',
                                          { paths, collectionId: col.id }
                                        );
                                        await loadLibrary();
                                        await loadCollections();
                                        const books = await invoke<LibraryBook[]>('reader_get_collection_books', { collectionId: col.id });
                                        setCollectionBooks(books);
                                      } finally {
                                        setImporting(false);
                                      }
                                    }
                                  } catch (err) {
                                    console.error(err);
                                  }
                                }}
                                title="Pick individual book files and add them to this collection"
                              >
                                <svg class="w-4 h-4 mr-1.5 inline-block" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 13h6m-3-3v6m-9 1V7a2 2 0 012-2h6l2 2h6a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z" />
                                </svg>
                                Add Files
                              </button>
                            </div>

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
                                  <div class="text-center py-8">
                                    <svg class="w-12 h-12 mx-auto mb-2 text-gray-300 dark:text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
                                    </svg>
                                    <p class="text-sm text-gray-500">This collection is empty.</p>
                                    <p class="text-xs text-gray-400 mt-1">
                                      Use the buttons above to add books.
                                    </p>
                                  </div>
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
        {/* ADD EXISTING BOOKS TO COLLECTION MODAL                          */}
        {/* ================================================================ */}
        <Show when={addBooksCollectionId()}>
          <div
            class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
            onClick={(e) => {
              if (e.target === e.currentTarget) closeAddBooksPicker();
            }}
          >
            <div class="card w-full max-w-2xl max-h-[80vh] flex flex-col shadow-2xl">
              <div class="p-5 border-b border-gray-200 dark:border-gray-700">
                <div class="flex items-start justify-between mb-3">
                  <div>
                    <h2 class="text-xl font-bold">Add Books to Collection</h2>
                    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
                      Select existing books from your library to add to{' '}
                      <span class="font-medium">
                        {collections().find((c) => c.id === addBooksCollectionId())?.name}
                      </span>
                    </p>
                  </div>
                  <button
                    class="p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
                    onClick={closeAddBooksPicker}
                  >
                    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                  </button>
                </div>
                <input
                  type="text"
                  class="input text-sm w-full"
                  placeholder="Search by title, author, or path..."
                  value={addBooksFilter()}
                  onInput={(e) => setAddBooksFilter(e.currentTarget.value)}
                />
              </div>

              <div class="flex-1 overflow-auto p-2">
                <Show
                  when={booksNotInCollection().length > 0}
                  fallback={
                    <div class="text-center py-12 text-gray-500">
                      <Show when={libraryBooks().length === 0} fallback={
                        <p>All books are already in this collection, or no matches for your search.</p>
                      }>
                        <p>Your library is empty.</p>
                        <p class="text-xs mt-1">Add books via "Add Files" or "Add Folder" first.</p>
                      </Show>
                    </div>
                  }
                >
                  <div class="space-y-0.5">
                    <For each={booksNotInCollection()}>
                      {(book) => (
                        <label class="flex items-center gap-3 px-3 py-2 rounded hover:bg-gray-50 dark:hover:bg-gray-800/50 cursor-pointer">
                          <input
                            type="checkbox"
                            class="w-4 h-4 rounded"
                            checked={addBooksSelected().has(book.id)}
                            onChange={() => toggleAddBookSelection(book.id)}
                          />
                          <span class="text-xs font-mono uppercase bg-gray-200 dark:bg-gray-700 rounded px-1.5 py-0.5 min-w-[40px] text-center">
                            {book.format || '?'}
                          </span>
                          <div class="flex-1 min-w-0">
                            <p class="text-sm truncate font-medium">
                              {book.title || book.file_path.split('/').pop() || 'Untitled'}
                            </p>
                            <Show when={book.authors}>
                              <p class="text-xs text-gray-500 truncate">{book.authors}</p>
                            </Show>
                          </div>
                        </label>
                      )}
                    </For>
                  </div>
                </Show>
              </div>

              <div class="p-4 border-t border-gray-200 dark:border-gray-700 bg-gray-50/50 dark:bg-gray-800/30 flex justify-between items-center">
                <span class="text-xs text-gray-500">
                  {addBooksSelected().size} selected
                </span>
                <div class="flex gap-2">
                  <button class="btn btn-secondary text-sm" onClick={closeAddBooksPicker}>
                    Cancel
                  </button>
                  <button
                    class="btn btn-primary text-sm"
                    onClick={confirmAddBooksToCollection}
                    disabled={addBooksSelected().size === 0}
                  >
                    Add {addBooksSelected().size > 0 ? `${addBooksSelected().size} ` : ''}
                    book{addBooksSelected().size === 1 ? '' : 's'}
                  </button>
                </div>
              </div>
            </div>
          </div>
        </Show>

        {/* ================================================================ */}
        {/* IMPORT MODAL (checkbox file selection)                           */}
        {/* ================================================================ */}
        <Show when={showImportModal()}>
          <div
            class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
            onClick={(e) => {
              if (e.target === e.currentTarget) setShowImportModal(false);
            }}
          >
            <div class="card w-full max-w-3xl max-h-[85vh] flex flex-col shadow-2xl">
              {/* Modal header */}
              <div class="p-5 border-b border-gray-200 dark:border-gray-700">
                <div class="flex items-start justify-between mb-2">
                  <div>
                    <h2 class="text-xl font-bold">Import Books from Folder</h2>
                    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1 truncate max-w-xl" title={importModalPath()}>
                      {importModalPath()}
                    </p>
                  </div>
                  <button
                    class="p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
                    onClick={() => setShowImportModal(false)}
                  >
                    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                  </button>
                </div>

                <Show when={!importLoading() && importCandidates().length > 0}>
                  <div class="flex items-center gap-2 flex-wrap">
                    <input
                      type="text"
                      class="input text-sm flex-1 min-w-[180px]"
                      placeholder="Filter by filename or extension..."
                      value={importFilter()}
                      onInput={(e) => setImportFilter(e.currentTarget.value)}
                    />
                    <button class="btn btn-secondary text-xs" onClick={selectAllImport}>
                      Select All
                    </button>
                    <button class="btn btn-secondary text-xs" onClick={deselectAllImport}>
                      Clear
                    </button>
                    <span class="text-xs text-gray-500 ml-1">
                      {importSelected().size} / {importCandidates().filter((c) => !c.already_imported).length} selected
                    </span>
                  </div>
                </Show>
              </div>

              {/* Modal body: file list */}
              <div class="flex-1 overflow-auto p-2">
                <Show when={importLoading()}>
                  <div class="text-center py-12 text-gray-500">
                    <div class="w-8 h-8 mx-auto mb-3 rounded-full border-2 border-gray-200 dark:border-gray-700 border-t-minion-500" style={{ animation: 'spin 1s linear infinite' }} />
                    Scanning folder...
                  </div>
                </Show>

                <Show when={!importLoading() && importCandidates().length === 0}>
                  <div class="text-center py-12 text-gray-500">
                    <p>No supported book files found in this folder.</p>
                    <p class="text-xs mt-1">Supported: EPUB, PDF, MOBI, TXT, MD, HTML</p>
                  </div>
                </Show>

                <Show when={!importLoading() && importCandidates().length > 0}>
                  <div class="space-y-0.5">
                    <For each={filteredImportCandidates()}>
                      {(file) => (
                        <label
                          class="flex items-center gap-3 px-3 py-2 rounded hover:bg-gray-50 dark:hover:bg-gray-800/50 cursor-pointer"
                          classList={{
                            'opacity-50': file.already_imported,
                          }}
                        >
                          <input
                            type="checkbox"
                            class="w-4 h-4 rounded"
                            checked={importSelected().has(file.path)}
                            disabled={file.already_imported}
                            onChange={() => toggleImportSelection(file.path)}
                          />
                          <span class="text-xs font-mono uppercase bg-gray-200 dark:bg-gray-700 rounded px-1.5 py-0.5 min-w-[44px] text-center">
                            {file.extension}
                          </span>
                          <div class="flex-1 min-w-0">
                            <p class="text-sm truncate" title={file.path}>
                              {file.name}
                            </p>
                            <Show when={file.already_imported}>
                              <p class="text-xs text-amber-600 dark:text-amber-400">Already in library</p>
                            </Show>
                          </div>
                          <span class="text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap">
                            {formatBytes(file.size)}
                          </span>
                        </label>
                      )}
                    </For>
                  </div>
                </Show>
              </div>

              {/* Modal footer */}
              <div class="p-4 border-t border-gray-200 dark:border-gray-700 bg-gray-50/50 dark:bg-gray-800/30">
                <div class="flex items-center gap-3 mb-3">
                  <label class="text-sm text-gray-600 dark:text-gray-300 whitespace-nowrap">Add to collection:</label>
                  <select
                    class="input text-sm flex-1"
                    value={importTargetCollection()}
                    onChange={(e) => setImportTargetCollection(e.currentTarget.value)}
                  >
                    <option value="">— None —</option>
                    <For each={collections()}>
                      {(col) => <option value={col.id}>{col.name}</option>}
                    </For>
                  </select>
                </div>
                <div class="flex justify-end gap-2">
                  <button
                    class="btn btn-secondary text-sm"
                    onClick={() => setShowImportModal(false)}
                    disabled={importing()}
                  >
                    Cancel
                  </button>
                  <button
                    class="btn btn-primary text-sm"
                    onClick={confirmImportSelection}
                    disabled={importing() || importSelected().size === 0}
                  >
                    {importing() ? 'Importing...' : `Import ${importSelected().size} file${importSelected().size === 1 ? '' : 's'}`}
                  </button>
                </div>
              </div>
            </div>
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
                height: '2px',
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

            {/* Reader Header (Apple Books–like: back | centered title | tools) */}
            <div
              class="flex items-center gap-2 px-3 py-2.5 border-b"
              style={{
                background: modeStyles().headerBg,
                'border-color': modeStyles().headerBorder,
                color: modeStyles().text,
                'flex-shrink': '0',
              }}
            >
              <button
                class="p-2 rounded-lg transition-colors flex-shrink-0"
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

              <div class="flex-1 min-w-0 text-center px-1">
                <h2
                  class="text-lg font-semibold tracking-tight truncate"
                  style={{ color: modeStyles().text }}
                >
                  {currentBook()?.metadata.title || loadingBookMeta()?.title || 'Loading...'}
                </h2>
                <p class="text-[11px] sm:text-xs mt-0.5 truncate" style={{ color: modeStyles().mutedText }}>
                  {currentBook()
                    ? <>
                        {currentBook()!.metadata.authors?.length > 0
                          ? currentBook()!.metadata.authors.join(', ') + ' \u00B7 '
                          : ''}
                        {isPdf()
                          ? `Page ${pdfCurrentPage()} of ${pdfTotalPages()}`
                          : `Chapter ${currentChapter() + 1} of ${currentBook()!.chapters.length}`
                        }
                      </>
                    : loadingBookMeta()?.author || 'Loading book content...'}
                </p>
              </div>

              <div class="flex items-center gap-1.5 flex-shrink-0 justify-end">
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

                {/* Font size controls (hide for PDF since we use zoom) */}
                <Show when={!isPdf()}>
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
                </Show>

                {/* PDF: fit modes, zoom (actual), fullscreen */}
                <Show when={isPdf()}>
                  <div class="flex items-center gap-1 flex-wrap justify-end max-w-[min(100%,420px)]">
                    <button
                      type="button"
                      class={`text-[10px] sm:text-xs px-1.5 py-1 rounded-md transition-colors ${
                        pdfFitMode() === 'fitWidth' ? 'ring-1 ring-sky-500/60' : ''
                      }`}
                      style={{
                        background: modeStyles().hoverBg,
                        color: modeStyles().text,
                      }}
                      onClick={() => setPdfFitMode('fitWidth')}
                      title="Fit width"
                    >
                      Width
                    </button>
                    <button
                      type="button"
                      class={`text-[10px] sm:text-xs px-1.5 py-1 rounded-md transition-colors ${
                        pdfFitMode() === 'fitPage' ? 'ring-1 ring-sky-500/60' : ''
                      }`}
                      style={{
                        background: modeStyles().hoverBg,
                        color: modeStyles().text,
                      }}
                      onClick={() => setPdfFitMode('fitPage')}
                      title="Fit page"
                    >
                      Page
                    </button>
                    <button
                      type="button"
                      class={`text-[10px] sm:text-xs px-1.5 py-1 rounded-md transition-colors ${
                        pdfFitMode() === 'actual' ? 'ring-1 ring-sky-500/60' : ''
                      }`}
                      style={{
                        background: modeStyles().hoverBg,
                        color: modeStyles().text,
                      }}
                      onClick={() => setPdfFitMode('actual')}
                      title="Actual size (pinch-style zoom with +/-)"
                    >
                      100%
                    </button>
                    <button
                      class="zoom-btn"
                      style={{
                        background: modeStyles().hoverBg,
                        color: modeStyles().text,
                      }}
                      onClick={() => {
                        setPdfFitMode('actual');
                        setPdfZoom(Math.max(0.5, pdfZoom() - 0.25));
                      }}
                      title="Zoom out"
                    >
                      −
                    </button>
                    <span class="text-[10px] sm:text-xs min-w-[3.25rem] text-center tabular-nums" style={{ color: modeStyles().mutedText }}>
                      {pdfZoomLabel()}
                    </span>
                    <button
                      class="zoom-btn"
                      style={{
                        background: modeStyles().hoverBg,
                        color: modeStyles().text,
                      }}
                      onClick={() => {
                        setPdfFitMode('actual');
                        setPdfZoom(Math.min(4, pdfZoom() + 0.25));
                      }}
                      title="Zoom in"
                    >
                      +
                    </button>
                    <button
                      type="button"
                      class="p-2 rounded-lg transition-colors"
                      style={{ color: modeStyles().text }}
                      onMouseEnter={(e) => {
                        (e.currentTarget as HTMLElement).style.background = modeStyles().hoverBg;
                      }}
                      onMouseLeave={(e) => {
                        (e.currentTarget as HTMLElement).style.background = 'transparent';
                      }}
                      onClick={() => void toggleReaderFullscreen()}
                      title="Fullscreen"
                    >
                      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          stroke-width="2"
                          d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4"
                        />
                      </svg>
                    </button>
                  </div>
                </Show>

                {/* TOC toggle (hide for PDF) */}
                <Show when={!isPdf()}>
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
                </Show>

                {/* Fullscreen toggle */}
                <button
                  class="p-2 rounded-lg transition-colors"
                  style={{ color: modeStyles().text }}
                  onMouseEnter={(e) => {
                    (e.currentTarget as HTMLElement).style.background = modeStyles().hoverBg;
                  }}
                  onMouseLeave={(e) => {
                    (e.currentTarget as HTMLElement).style.background = 'transparent';
                  }}
                  onClick={toggleFullscreen}
                  title={isFullscreen() ? 'Exit fullscreen (Esc)' : 'Fullscreen (F11)'}
                >
                  <Show when={!isFullscreen()} fallback={
                    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 9L4 4m0 0v5m0-5h5m6 6l5 5m0 0v-5m0 5h-5M9 15l-5 5m0 0h5m-5 0v-5m11-6l5-5m0 0h-5m5 0v5" />
                    </svg>
                  }>
                    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 8V4m0 0h4M4 4l5 5m11-5h-4m4 0v4m0 0l-5-5m-7 14H4m0 0v-4m0 4l5-5m7 5h4m0 0v-4m0 4l-5-5" />
                    </svg>
                  </Show>
                </button>
              </div>
            </div>

            {/* Content area */}
            <div class="flex-1 flex overflow-hidden" style={{ 'min-height': '0' }}>
              {/* TOC Sidebar (EPUB / TXT / MD / HTML only) */}
              <Show when={showToc() && currentBook() && !isPdf()}>
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

              {/* ============================================================ */}
              {/* PDF Content (canvas-based via pdf.js)                        */}
              {/* ============================================================ */}
              <Show when={isPdf()}>
                <div
                  ref={(el) => {
                    pdfContainerRef = el;
                    pdfResizeObserver?.disconnect();
                    if (el) {
                      pdfResizeObserver = new ResizeObserver(() => {
                        setPdfLayoutTick(t => t + 1);
                      });
                      pdfResizeObserver.observe(el);
                    }
                  }}
                  data-reading-scroll
                  class="flex-1 overflow-auto overflow-x-hidden"
                  onScroll={handleReadScroll}
                  style={{
                    background: readingMode() === 'dark' ? '#1a1a2e'
                      : readingMode() === 'sepia' ? '#f4ecd8'
                      : '#e5e7eb',
                  }}
                >
                  <Show when={pdfLoading()}>
                    <div class="flex flex-col items-center justify-center py-24">
                      <div class="chapter-spinner mb-4" />
                      <p class="text-sm" style={{ color: modeStyles().mutedText }}>
                        Loading PDF...
                      </p>
                    </div>
                  </Show>
                  <Show when={!pdfLoading() && pdfDoc()}>
                    <div class="pdf-canvas-container py-6 px-4 flex justify-center">
                      <div class="apple-books-read-layer rounded-md">
                        <div class={`pdf-page-swap ${pdfPageSwapClass()}`}>
                          <canvas
                            ref={(el) => {
                              pdfCanvasRef = el;
                              if (!el) return;
                              queueMicrotask(() => {
                                void renderPdfPage(pdfCurrentPage());
                              });
                            }}
                            style={{
                              'max-width': '100%',
                              'height': 'auto',
                              display: 'block',
                            }}
                          />
                        </div>
                      </div>
                    </div>
                  </Show>
                  <Show when={!pdfLoading() && !pdfDoc()}>
                    <div class="flex flex-col items-center justify-center py-24">
                      <p class="text-sm" style={{ color: modeStyles().mutedText }}>
                        Failed to load PDF. The file may be corrupted.
                      </p>
                    </div>
                  </Show>
                </div>
              </Show>

              {/* ============================================================ */}
              {/* EPUB / TXT / MD / HTML Content                               */}
              {/* ============================================================ */}
              <Show when={!isPdf()}>
                <div
                  ref={(el) => {
                    textScrollContainerRef = el;
                  }}
                  data-reading-scroll
                  class="flex-1 overflow-y-auto font-sans"
                  classList={{
                    'overflow-x-hidden': !epubTurnOutgoingHtml(),
                    'epub-page-turn-scroll': !!epubTurnOutgoingHtml(),
                  }}
                  onScroll={handleReadScroll}
                  style={{
                    background: modeStyles().contentBg,
                    color: modeStyles().text,
                  }}
                >
                  <div class="reader-chapter-perspective reader-chapter-stage min-h-full px-3 sm:px-6 py-1">
                    <div
                      class="max-w-[min(68ch,42rem)] mx-auto px-5 sm:px-8 py-12 sm:py-14"
                      style={{ 'font-size': `${fontSize()}px` }}
                      classList={{
                        'page-exit-left':
                          !isEpub() && pageTransitioning() && pageDirection() === 'left',
                        'page-enter-right':
                          !isEpub() && !pageTransitioning() && pageDirection() === 'left',
                        'page-exit-right':
                          !isEpub() && pageTransitioning() && pageDirection() === 'right',
                        'page-enter-left':
                          !isEpub() && !pageTransitioning() && pageDirection() === 'right',
                      }}
                    >
                      <div class="apple-books-read-layer rounded-sm">
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
                      {/* EPUB content (lazy loaded) — 3D page turn between chapters */}
                      <Show when={isEpub()}>
                        <Show when={chapterLoading() && !chapterHtml() && !epubTurnOutgoingHtml()}>
                          <div class="flex flex-col items-center justify-center py-16">
                            <div class="chapter-spinner mb-4" />
                            <p class="text-sm" style={{ color: modeStyles().mutedText }}>
                              Loading chapter...
                            </p>
                          </div>
                        </Show>
                        <Show when={!!(epubTurnOutgoingHtml() && epubTurnIncomingHtml())}>
                          <div class="epub-page-turn-stage">
                            <EpubStPageFlip
                              dir={epubTurnDir()}
                              outgoingTitle={chapterTitleForIndex(currentChapter())}
                              incomingTitle={chapterTitleForIndex(epubTurnTargetIndex() ?? 0)}
                              outgoingHtml={epubTurnOutgoingHtml()}
                              incomingHtml={epubTurnIncomingHtml()}
                              proseClass={modeStyles().prose}
                              chapterTitleColor={modeStyles().chapterTitle}
                              onComplete={() => {
                                const t = epubTurnTargetIndex();
                                const inc = epubTurnIncomingHtml();
                                if (t === null || !inc) return;
                                completeEpubChapterTurn(t, inc);
                              }}
                            />
                          </div>
                        </Show>
                        <Show when={!epubTurnOutgoingHtml() && chapterHtml()}>
                          <h1
                            class="text-2xl font-bold mb-8"
                            style={{
                              color: modeStyles().chapterTitle,
                              'letter-spacing': '-0.02em',
                            }}
                          >
                            {currentBook()!.chapters[currentChapter()]?.title ||
                              `Chapter ${currentChapter() + 1}`}
                          </h1>
                          <div
                            class={`prose max-w-none ${modeStyles().prose}`}
                            innerHTML={chapterHtml()}
                          />
                        </Show>
                      </Show>

                      {/* TXT / MD / HTML content (full content in chapters[0]) */}
                      <Show when={!isEpub() && currentBook()!.chapters[currentChapter()]}>
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
                </div>
              </Show>
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
                {/* PDF navigation */}
                <Show when={isPdf()}>
                  <button
                    class="btn nav-btn"
                    style={
                      {
                        background: modeStyles().hoverBg,
                        color: modeStyles().text,
                        '--nav-hover-x': '-3px',
                        opacity: pdfCurrentPage() <= 1 || pdfNavBusy() ? '0.35' : '1',
                        cursor: pdfCurrentPage() <= 1 || pdfNavBusy() ? 'not-allowed' : 'pointer',
                      } as any
                    }
                    onClick={prevPdfPage}
                    disabled={pdfCurrentPage() <= 1 || pdfNavBusy()}
                  >
                    <span class="flex items-center gap-1.5">
                      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
                      </svg>
                      Previous
                    </span>
                  </button>

                  <div class="flex items-center gap-3">
                    <span class="text-sm" style={{ color: modeStyles().mutedText }}>Page</span>
                    <input
                      type="text"
                      class="w-14 text-center text-sm rounded-md border px-1 py-0.5"
                      style={{
                        background: modeStyles().hoverBg,
                        color: modeStyles().text,
                        'border-color': modeStyles().headerBorder,
                      }}
                      value={pdfPageInputValue()}
                      onInput={(e) => setPdfPageInputValue(e.currentTarget.value)}
                      onKeyDown={handlePdfPageInput}
                      onBlur={() => {
                        const val = parseInt(pdfPageInputValue(), 10);
                        if (!isNaN(val) && val >= 1 && val <= pdfTotalPages()) {
                          goToPdfPage(val);
                        } else {
                          setPdfPageInputValue(String(pdfCurrentPage()));
                        }
                      }}
                    />
                    <span class="text-sm" style={{ color: modeStyles().mutedText }}>
                      of {pdfTotalPages()}
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
                        opacity: pdfCurrentPage() >= pdfTotalPages() || pdfNavBusy() ? '0.35' : '1',
                        cursor: pdfCurrentPage() >= pdfTotalPages() || pdfNavBusy() ? 'not-allowed' : 'pointer',
                      } as any
                    }
                    onClick={nextPdfPage}
                    disabled={pdfCurrentPage() >= pdfTotalPages() || pdfNavBusy()}
                  >
                    <span class="flex items-center gap-1.5">
                      Next
                      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
                      </svg>
                    </span>
                  </button>
                </Show>

                {/* EPUB / TXT / MD / HTML navigation */}
                <Show when={!isPdf()}>
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
                </Show>
              </div>
            </Show>
          </div>
        </Show>
      </div>
    </>
  );
};

export default Reader;
