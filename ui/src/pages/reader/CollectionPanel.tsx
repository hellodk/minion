import { Component, createSignal, createEffect, on, For, Show, JSX, Accessor } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type { LibraryBook, Collection } from './types';

// ---- Preset colors (same as Reader.tsx) ----

const PRESET_COLORS = [
  '#0ea5e9', '#8b5cf6', '#ec4899', '#f97316', '#22c55e',
  '#ef4444', '#14b8a6', '#f59e0b', '#6366f1', '#64748b',
];

// ---- Props ----

interface CollectionPanelProps {
  collections: Accessor<Collection[]>;
  libraryBooks: Accessor<LibraryBook[]>;
  onCollectionsChange: () => void;
  onLibraryChange: () => void;
  onImportFolderForCollection: (collectionId: string) => void;
  renderBookCard: (book: LibraryBook, index: number, opts?: { showRemoveFromCollection?: string }) => JSX.Element;
}

// ---- Component ----

const CollectionPanel: Component<CollectionPanelProps> = (props) => {
  // Collection creation form
  const [showNewCollection, setShowNewCollection] = createSignal(false);
  const [newCollectionName, setNewCollectionName] = createSignal('');
  const [newCollectionColor, setNewCollectionColor] = createSignal('#0ea5e9');
  const [creatingCollection, setCreatingCollection] = createSignal(false);

  // Collection detail view
  const [expandedCollection, setExpandedCollection] = createSignal<string | null>(null);
  const [collectionBooks, setCollectionBooks] = createSignal<LibraryBook[]>([]);
  const [loadingCollectionBooks, setLoadingCollectionBooks] = createSignal(false);

  // "Add existing books to collection" picker
  const [addBooksCollectionId, setAddBooksCollectionId] = createSignal<string | null>(null);
  const [addBooksSelected, setAddBooksSelected] = createSignal<Set<string>>(new Set<string>());
  const [addBooksFilter, setAddBooksFilter] = createSignal('');

  // "Add files" in-progress flag
  const [addingFiles, setAddingFiles] = createSignal(false);

  // Re-sync collection book list when parent collections update (e.g. after removal)
  createEffect(on(props.collections, async () => {
    const id = expandedCollection();
    if (!id) return;
    try {
      const books = await invoke<LibraryBook[]>('reader_get_collection_books', {
        collectionId: id,
      });
      setCollectionBooks(books);
    } catch {
      // silently ignore — list will refresh on next expansion
    }
  }));

  // ---- Collection CRUD ----

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
      props.onCollectionsChange();
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
      props.onCollectionsChange();
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

  // ---- Add existing books picker ----

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
    return props.libraryBooks().filter((b) => {
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
      props.onCollectionsChange();
      if (expandedCollection() === collectionId) {
        const books = await invoke<LibraryBook[]>('reader_get_collection_books', { collectionId });
        setCollectionBooks(books);
      }
      closeAddBooksPicker();
    } catch (e) {
      console.error('Failed to add books to collection:', e);
      alert(`Failed to add books: ${e}`);
    }
  };

  // ---- Add files directly to a collection ----

  const addFilesToCollection = async (collectionId: string) => {
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
        setAddingFiles(true);
        try {
          await invoke<{ imported: number; skipped: number; failed: number }>(
            'reader_import_paths',
            { paths, collectionId }
          );
          props.onLibraryChange();
          props.onCollectionsChange();
          const books = await invoke<LibraryBook[]>('reader_get_collection_books', { collectionId });
          setCollectionBooks(books);
        } finally {
          setAddingFiles(false);
        }
      }
    } catch (err) {
      console.error(err);
    }
  };

  // ---- Render ----

  return (
    <>
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
                  onKeyDown={(e) => e.key === 'Enter' && createCollection()}
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
        when={props.collections().length > 0}
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
          <For each={props.collections()}>
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
                        onClick={(e) => {
                          e.stopPropagation();
                          props.onImportFolderForCollection(col.id);
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
                          await addFilesToCollection(col.id);
                        }}
                        disabled={addingFiles()}
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
                              props.renderBookCard(book, index(), {
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
                      {props.collections().find((c) => c.id === addBooksCollectionId())?.name}
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
                    <Show when={props.libraryBooks().length === 0} fallback={
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
    </>
  );
};

export { CollectionPanel };
export type { CollectionPanelProps };
export type { Collection, LibraryBook } from './types';
