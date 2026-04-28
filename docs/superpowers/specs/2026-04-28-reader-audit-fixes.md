# Reader Section — Audit Fixes & Thumbnails

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix all bugs, performance bottlenecks, and architectural issues found in the Reader section audit, and complete the thumbnail feature for book covers (EPUB batch + PDF first-page).

**Architecture:** Fixes span three layers — Rust backend (commands.rs, migrations), SolidJS frontend (Reader.tsx → split into sub-components), and one new Tauri command (`reader_save_cover`). No new crates except `ammonia` for HTML sanitization.

**Tech Stack:** Rust (rusqlite, epub, base64ct, ammonia), SolidJS, Tauri 2 (`convertFileSrc`, asset protocol), pdf.js (already in project).

---

## Deferred (out of scope for this plan)

- **A1 — minion-reader crate wiring:** Dead stub crate. Defer — too large, no user-visible impact.
- **A5 — O'Reilly Chrome cookie reader:** Fragile by design. Defer — needs a full OAuth flow replacement.
- **P2 — EpubDoc session cache:** Cross-request state in Tauri is complex; the OS page cache already amortizes repeated opens. Defer until profiling proves it's a bottleneck.

---

## File Map

### Modified — Rust
- `src-tauri/src/commands.rs` — bug fixes, new `reader_save_cover` command, cover serving change, resource lookup fix, temp dir cleanup, remove ghost `reader_list_books`
- `crates/minion-db/src/migrations.rs` — add DB index on `reader_books.file_path`, batch file_path query fix is in commands.rs

### Modified — Frontend
- `ui/src/pages/Reader.tsx` — stripped to ~1500 lines: library state + routing only; delegates rendering to sub-components
- `ui/src/pages/reader/LibraryGrid.tsx` — **new** book card grid, cover display, tilt, collection dropdown
- `ui/src/pages/reader/EpubReader.tsx` — **new** EPUB chapter navigation, page-flip animation, reading modes
- `ui/src/pages/reader/PdfReader.tsx` — **new** pdf.js rendering, zoom, page nav, PDF thumbnail on import
- `ui/src/pages/reader/CollectionPanel.tsx` — **new** collection list/detail, folder import modal

### New dependency
- `Cargo.toml` (minion-reader or src-tauri) — add `ammonia = "4"` for HTML sanitization

---

## Task 1: DB migration — index on `reader_books.file_path`

**Files:**
- Modify: `crates/minion-db/src/migrations.rs`

- [ ] **Step 1: Find the next migration number**

```bash
grep -c "fn migrate_" crates/minion-db/src/migrations.rs
```

Expected: a number N. The new function will be `migrate_0NN_reader_index`.

- [ ] **Step 2: Add the migration function**

In `crates/minion-db/src/migrations.rs`, after the last `migrate_*` function, add:

```rust
fn migrate_021_reader_file_path_index(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_reader_books_file_path
         ON reader_books(file_path);",
    )?;
    Ok(())
}
```

- [ ] **Step 3: Register it in `run_migrations`**

Find the `run_migrations` function and add the call in sequence:

```rust
migrate_021_reader_file_path_index(conn)?;
```

- [ ] **Step 4: Verify compilation**

```bash
cargo build -p minion-db 2>&1 | tail -5
```
Expected: `Finished` with no errors.

- [ ] **Step 5: Commit**

```bash
git add crates/minion-db/src/migrations.rs
git commit -m "fix(reader): unique index on reader_books.file_path — prevents duplicates, speeds up import checks"
```

---

## Task 2: Fix `reader_list_folder_files` — batch file_path query

**Files:**
- Modify: `src-tauri/src/commands.rs` (around line 4173)

- [ ] **Step 1: Replace N individual queries with one batch query**

Find the block at line ~4173:
```rust
for c in candidates.iter_mut() {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM reader_books WHERE file_path = ?1)",
            rusqlite::params![c.path],
            |row| row.get(0),
        )
        .unwrap_or(false);
    c.already_imported = exists;
}
```

Replace with:

```rust
// Collect all candidate paths and batch-check against the DB
let all_paths: Vec<&str> = candidates.iter().map(|c| c.path.as_str()).collect();

// SQLite IN clause with placeholders
let placeholders = all_paths
    .iter()
    .enumerate()
    .map(|(i, _)| format!("?{}", i + 1))
    .collect::<Vec<_>>()
    .join(", ");

let query = format!(
    "SELECT file_path FROM reader_books WHERE file_path IN ({})",
    placeholders
);

let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;
let imported_paths: std::collections::HashSet<String> = stmt
    .query_map(rusqlite::params_from_iter(all_paths.iter()), |row| {
        row.get::<_, String>(0)
    })
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();

for c in candidates.iter_mut() {
    c.already_imported = imported_paths.contains(&c.path);
}
```

- [ ] **Step 2: Test it compiles**

```bash
cargo build -p minion-app 2>&1 | tail -5
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "perf(reader): batch file_path query in reader_list_folder_files — O(1) instead of O(n) DB calls"
```

---

## Task 3: Cover serving — remove base64 re-encoding, use file paths

**Files:**
- Modify: `src-tauri/src/commands.rs` (`reader_get_library`, `reader_get_collection_books`)

- [ ] **Step 1: Remove base64 re-encoding from `reader_get_library`**

Find the post-processing loop (~line 3799):
```rust
for row in rows {
    let mut book = row.map_err(|e| e.to_string())?;
    // Convert cover file path to base64 data URI for the frontend
    if let Some(ref path) = book.cover_path {
        if !path.starts_with("data:") && std::path::Path::new(path).exists() {
            if let Ok(data) = std::fs::read(path) {
                use base64ct::{Base64, Encoding};
                let b64 = Base64::encode_string(&data);
                let mime = if path.ends_with(".png") { "image/png" } else { "image/jpeg" };
                book.cover_path = Some(format!("data:{};base64,{}", mime, b64));
            }
        }
    }
    books.push(book);
}
```

Replace with (just return the path directly):
```rust
for row in rows {
    books.push(row.map_err(|e| e.to_string())?);
}
```

- [ ] **Step 2: Do the same in `reader_get_collection_books`** (~line 4084)

Find the identical loop and remove it:
```rust
// Remove the base64 re-encoding loop here too — just push row directly
for row in rows {
    books.push(row.map_err(|e| e.to_string())?);
}
```

- [ ] **Step 3: Check for any other cover re-encoding loops**

```bash
grep -n "base64.*cover\|cover.*base64\|data:image.*base64" src-tauri/src/commands.rs
```

Remove any remaining re-encoding that isn't the import step (the `reader_import_book` cover save is fine — it writes the file, doesn't re-encode).

- [ ] **Step 4: Compile check**

```bash
cargo build -p minion-app 2>&1 | tail -5
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "perf(reader): remove base64 re-encoding of covers in reader_get_library — return file paths instead"
```

---

## Task 4: Frontend — use `convertFileSrc` for cover images

**Files:**
- Modify: `ui/src/pages/Reader.tsx`

- [ ] **Step 1: Import `convertFileSrc`**

At the top of `Reader.tsx`, add:
```typescript
import { convertFileSrc } from '@tauri-apps/api/core';
```

- [ ] **Step 2: Add a helper to get the displayable cover URL**

After the existing imports/types block, add:
```typescript
function coverUrl(path: string | undefined): string | undefined {
  if (!path) return undefined;
  if (path.startsWith('data:') || path.startsWith('http')) return path;
  return convertFileSrc(path);
}
```

- [ ] **Step 3: Update `renderBookCard` to use the helper**

Find (~line 1514):
```tsx
<Show when={book.cover_path} fallback={...}>
  <img
    src={book.cover_path!}
```

Replace `src={book.cover_path!}` with:
```tsx
src={coverUrl(book.cover_path)!}
```

- [ ] **Step 4: Check for any other places that use `cover_path` as a src**

```bash
grep -n "cover_path\|cover_base64" ui/src/pages/Reader.tsx | grep -v "//\|show\|Show\|set\|get\|?.cover"
```

Update all `src=` uses of cover paths to go through `coverUrl()`.

- [ ] **Step 5: TypeScript check**

```bash
cd ui && pnpm typecheck 2>&1 | tail -10
```
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add ui/src/pages/Reader.tsx
git commit -m "perf(reader): use convertFileSrc for cover images — browser caches file:// URLs, eliminates base64 IPC overhead"
```

---

## Task 5: Fix cover extraction in batch import (`reader_import_paths`)

**Files:**
- Modify: `src-tauri/src/commands.rs` (~line 4270)

- [ ] **Step 1: Add cover extraction to the EPUB import path inside `reader_import_paths`**

Find the block inside `reader_import_paths` where a new book is inserted (~line 4272):

```rust
let id = uuid::Uuid::new_v4().to_string();
let now = chrono::Utc::now().to_rfc3339();

if conn
    .execute(
        "INSERT INTO reader_books (id, title, authors, file_path, format, progress, added_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
        rusqlite::params![id, title, authors, p, ext, now],
    )
    .is_err()
{
```

Replace with cover extraction before the insert:

```rust
let id = uuid::Uuid::new_v4().to_string();
let now = chrono::Utc::now().to_rfc3339();

// Extract cover for EPUB files
let cover_path: Option<String> = if ext == "epub" {
    if let Ok(mut doc) = epub::doc::EpubDoc::new(&book_path) {
        if let Some((cover_data, _mime)) = doc.get_cover() {
            let covers_dir = st.data_dir.join("covers");
            let _ = std::fs::create_dir_all(&covers_dir);
            let cover_file = covers_dir.join(format!("{}.jpg", id));
            if std::fs::write(&cover_file, &cover_data).is_ok() {
                Some(cover_file.to_string_lossy().to_string())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
} else {
    None
};

if conn
    .execute(
        "INSERT INTO reader_books (id, title, authors, file_path, format, cover_path, progress, added_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
        rusqlite::params![id, title, authors, p, ext, cover_path, now],
    )
    .is_err()
{
```

- [ ] **Step 2: Compile check**

```bash
cargo build -p minion-app 2>&1 | tail -5
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "fix(reader): extract EPUB cover in batch import — reader_import_paths now saves covers same as reader_import_book"
```

---

## Task 6: New command `reader_save_cover` (for PDF thumbnail)

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add the command**

After `reader_update_progress`, add:

```rust
/// Save a cover image (JPEG bytes) for a book — used by the frontend
/// after rendering a PDF page 1 via pdf.js to a canvas.
#[tauri::command]
pub async fn reader_save_cover(
    state: State<'_, AppStateHandle>,
    book_id: String,
    jpeg_bytes: Vec<u8>,
) -> Result<String, String> {
    let st = state.read().await;
    let covers_dir = st.data_dir.join("covers");
    std::fs::create_dir_all(&covers_dir).map_err(|e| e.to_string())?;

    let cover_file = covers_dir.join(format!("{}.jpg", book_id));
    std::fs::write(&cover_file, &jpeg_bytes).map_err(|e| e.to_string())?;

    let cover_path = cover_file.to_string_lossy().to_string();

    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE reader_books SET cover_path = ?1 WHERE id = ?2",
        rusqlite::params![cover_path, book_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(cover_path)
}
```

- [ ] **Step 2: Register in `lib.rs`**

Find the `invoke_handler` list in `src-tauri/src/lib.rs` and add:
```rust
commands::reader_save_cover,
```

- [ ] **Step 3: Compile check**

```bash
cargo build -p minion-app 2>&1 | tail -5
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(reader): add reader_save_cover command — stores JPEG bytes from frontend PDF thumbnail render"
```

---

## Task 7: PDF thumbnail — frontend render on import

**Files:**
- Modify: `ui/src/pages/Reader.tsx`

- [ ] **Step 1: Add `generatePdfThumbnail` helper function**

After the `coverUrl()` helper from Task 4, add:

```typescript
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

    const scale = 0.5; // ~150px wide for a 300pt page
    const viewport = page.getViewport({ scale });

    const canvas = document.createElement('canvas');
    canvas.width = viewport.width;
    canvas.height = viewport.height;
    const ctx = canvas.getContext('2d')!;

    await page.render({ canvasContext: ctx, viewport }).promise;

    // Convert canvas to JPEG bytes
    const blob = await new Promise<Blob>((res) =>
      canvas.toBlob((b) => res(b!), 'image/jpeg', 0.82)
    );
    const arrayBuffer = await blob.arrayBuffer();
    const jpegBytes = Array.from(new Uint8Array(arrayBuffer));

    await invoke('reader_save_cover', { bookId, jpegBytes });
    await pdf.destroy();
  } catch (e) {
    console.warn('PDF thumbnail generation failed:', e);
  }
}
```

- [ ] **Step 2: Call thumbnail generation after PDF import**

Find the `openLibraryBook` function (or wherever `reader_import_book` is called for PDFs). After a successful import of a PDF, call:

```typescript
// Fire-and-forget — don't block opening the book
if (ext === 'pdf' && !imported.cover_path) {
  void generatePdfThumbnail(imported.file_path, imported.id);
}
```

Also call it after `reader_import_paths` completes, for any newly imported PDFs:

```typescript
// After reader_import_paths returns, reload library and generate missing PDF thumbnails
await loadLibrary();
const pdfsNeedingThumbs = libraryBooks().filter(
  b => (b.format === 'pdf') && !b.cover_path
);
for (const b of pdfsNeedingThumbs) {
  void generatePdfThumbnail(b.file_path, b.id);
}
```

- [ ] **Step 3: TypeScript check**

```bash
cd ui && pnpm typecheck 2>&1 | tail -10
```

- [ ] **Step 4: Commit**

```bash
git add ui/src/pages/Reader.tsx
git commit -m "feat(reader): generate PDF thumbnail on import via pdf.js canvas render"
```

---

## Task 8: Remove ghost command `reader_list_books`

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Delete `reader_list_books` and `BookInfo` struct**

In `commands.rs`, find and delete:
- `pub struct BookInfo { ... }` (~line 1483)
- `pub async fn reader_list_books(directory: String) -> ...` (~line 1983)

- [ ] **Step 2: Remove from `lib.rs`**

```bash
grep -n "reader_list_books" src-tauri/src/lib.rs
```

Remove the line.

- [ ] **Step 3: Check nothing else uses it**

```bash
grep -rn "reader_list_books\|BookInfo" ui/src/ src-tauri/src/
```

Expected: no results.

- [ ] **Step 4: Compile**

```bash
cargo build -p minion-app 2>&1 | tail -5
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "refactor(reader): remove unused reader_list_books command and BookInfo struct"
```

---

## Task 9: Remove unsupported formats from folder picker

**Files:**
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: Trim `book_extensions` in `reader_list_folder_files`**

Find (~line 4136):
```rust
let book_extensions = [
    "epub", "pdf", "mobi", "azw3", "fb2", "djvu", "cbz", "cbr", "txt", "md", "markdown",
    "html", "htm",
];
```

Replace with only formats that `reader_open_book` can actually handle:
```rust
let book_extensions = ["epub", "pdf", "txt", "md", "markdown", "html", "htm"];
```

- [ ] **Step 2: Do the same in `reader_scan_directory`** (~line 4324)

Same change: remove `mobi, azw3, fb2, djvu, cbz, cbr`.

- [ ] **Step 3: Compile**

```bash
cargo build -p minion-app 2>&1 | tail -5
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "fix(reader): remove unsupported formats (mobi/azw3/fb2/cbz/cbr) from folder picker — only show openable formats"
```

---

## Task 10: Fix EPUB image resource lookup (O(n×m) → O(n+m))

**Files:**
- Modify: `src-tauri/src/commands.rs` — `replace_epub_images_with_temp_files`

- [ ] **Step 1: Build a complete resource lookup map once at the top**

Find the function `replace_epub_images_with_temp_files` (~line 1826). Replace the per-image resource search loop with a pre-built map:

```rust
fn replace_epub_images_with_temp_files(
    html: &str,
    doc: &mut epub::doc::EpubDoc<std::io::BufReader<std::fs::File>>,
    book_path: &Path,
) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::collections::HashMap;

    let mut hasher = DefaultHasher::new();
    book_path.to_string_lossy().hash(&mut hasher);
    let book_hash = format!("{:x}", hasher.finish());

    let img_dir = std::env::temp_dir()
        .join("minion_book_images")
        .join(&book_hash);
    let _ = std::fs::create_dir_all(&img_dir);

    // Build a complete name→id map once (O(m)) instead of O(m) per image
    let mut name_to_id: HashMap<String, String> = HashMap::new();
    for (id, item) in doc.resources.iter() {
        let path_str = item.path.to_string_lossy().to_string();
        name_to_id.insert(path_str, id.clone());
        if let Some(fname) = item.path.file_name() {
            name_to_id.entry(fname.to_string_lossy().to_string()).or_insert_with(|| id.clone());
        }
    }

    let mut result = html.to_string();
    let mut search_start = 0;

    while let Some(src_pos) = result[search_start..].find("src=\"") {
        let abs_pos = search_start + src_pos + 4;
        let end_quote = match result[abs_pos..].find('"') {
            Some(p) => p,
            None => break,
        };
        let src_value = result[abs_pos..abs_pos + end_quote].to_string();

        if src_value.starts_with("data:")
            || src_value.starts_with("http")
            || src_value.starts_with("file:")
        {
            search_start = abs_pos + end_quote;
            continue;
        }

        let resource_name = src_value.rsplit('/').next().unwrap_or(&src_value).to_string();

        let resource_id = name_to_id.get(&src_value)
            .or_else(|| name_to_id.get(&resource_name))
            .cloned();

        if let Some(id) = resource_id {
            let img_path = img_dir.join(&resource_name);
            // Only write if not already extracted
            let file_url = if img_path.exists() || doc
                .get_resource(&id)
                .and_then(|(data, _)| std::fs::write(&img_path, &data).ok())
                .is_some()
            {
                format!("file://{}", img_path.to_string_lossy())
            } else {
                search_start = abs_pos + end_quote;
                continue;
            };

            result = format!(
                "{}{}{}",
                &result[..abs_pos],
                file_url,
                &result[abs_pos + end_quote..]
            );
            search_start = abs_pos + file_url.len();
        } else {
            search_start = abs_pos + end_quote;
        }
    }

    result
}
```

- [ ] **Step 2: Compile**

```bash
cargo build -p minion-app 2>&1 | tail -5
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "perf(reader): O(n+m) resource map in replace_epub_images_with_temp_files — was O(n×m)"
```

---

## Task 11: Temp image dir cleanup on book close

**Files:**
- Modify: `ui/src/pages/Reader.tsx` (or sub-components after Task 14)

- [ ] **Step 1: Add a new Tauri command `reader_cleanup_book_images`**

In `commands.rs`:

```rust
/// Delete extracted EPUB images for a book from the temp directory.
/// Called when the user closes a book.
#[tauri::command]
pub async fn reader_cleanup_book_images(book_path: String) -> Result<(), String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    book_path.hash(&mut hasher);
    let book_hash = format!("{:x}", hasher.finish());

    let img_dir = std::env::temp_dir()
        .join("minion_book_images")
        .join(&book_hash);

    if img_dir.exists() {
        std::fs::remove_dir_all(&img_dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}
```

Register it in `lib.rs`.

- [ ] **Step 2: Call cleanup when book is closed**

In `Reader.tsx`, find the `closeBook` function and add:

```typescript
const closeBook = () => {
  const book = currentBook();
  // Clean up temp images for the closed book
  if (book?.file_path && book.format === 'epub') {
    void invoke('reader_cleanup_book_images', { bookPath: book.file_path });
  }
  // ... existing close logic
};
```

- [ ] **Step 3: Also clean up on `onCleanup`**

```typescript
onCleanup(() => {
  const book = currentBook();
  if (book?.file_path && book.format === 'epub') {
    void invoke('reader_cleanup_book_images', { bookPath: book.file_path });
  }
  // ... existing cleanup
});
```

- [ ] **Step 4: TypeScript + compile check**

```bash
cargo build -p minion-app 2>&1 | tail -5 && cd ui && pnpm typecheck 2>&1 | tail -5
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs ui/src/pages/Reader.tsx
git commit -m "fix(reader): clean up temp EPUB image dir on book close — prevents unbounded /tmp growth"
```

---

## Task 12: Fix HTML sanitization — use `ammonia`

**Files:**
- Modify: `crates/minion-reader/Cargo.toml`
- Modify: `crates/minion-reader/src/formats.rs`

- [ ] **Step 1: Add ammonia dependency**

In `crates/minion-reader/Cargo.toml`:
```toml
ammonia = "4"
```

- [ ] **Step 2: Replace `sanitize_html`**

In `crates/minion-reader/src/formats.rs`, replace:

```rust
pub fn sanitize_html(html: &str) -> String {
    html.replace("<script", "<!--script")
        .replace("</script>", "</script-->")
}
```

With:

```rust
pub fn sanitize_html(html: &str) -> String {
    ammonia::clean(html)
}
```

- [ ] **Step 3: Check if `sanitize_html` is called from commands.rs**

```bash
grep -n "sanitize_html" src-tauri/src/commands.rs
```

If it is called there, the import chain works. If not, the function exists in the library crate but commands.rs may have its own sanitization — check and apply ammonia there too if needed.

- [ ] **Step 4: Compile**

```bash
cargo build --workspace 2>&1 | tail -5
```

- [ ] **Step 5: Commit**

```bash
git add crates/minion-reader/Cargo.toml crates/minion-reader/src/formats.rs
git commit -m "fix(reader): replace broken sanitize_html with ammonia — proper script/XSS removal"
```

---

## Task 13: Fix global tilt state — per-card tilt via CSS custom properties

**Files:**
- Modify: `ui/src/pages/Reader.tsx` (or `LibraryGrid.tsx` after split)

- [ ] **Step 1: Remove global tilt signals**

Delete these three signals from the component:
```typescript
const [hoveredCard, setHoveredCard] = createSignal<string | null>(null);
const [tiltX, setTiltX] = createSignal(0);
const [tiltY, setTiltY] = createSignal(0);
```

- [ ] **Step 2: Replace `handleCardMouseMove` and `handleCardMouseLeave` with CSS-based tilt**

Replace the mouse move handler with one that sets CSS custom properties directly on the card element — no reactive signals needed:

```typescript
const handleCardMouseMove = (e: MouseEvent) => {
  const card = (e.currentTarget as HTMLElement).querySelector('.book-card-inner') as HTMLElement;
  if (!card) return;
  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
  const x = (e.clientX - rect.left) / rect.width - 0.5;   // -0.5 to 0.5
  const y = (e.clientY - rect.top) / rect.height - 0.5;
  card.style.setProperty('--tilt-x', `${(-y * 12).toFixed(1)}deg`);
  card.style.setProperty('--tilt-y', `${(x * 12).toFixed(1)}deg`);
};

const handleCardMouseLeave = (e: MouseEvent) => {
  const card = (e.currentTarget as HTMLElement).querySelector('.book-card-inner') as HTMLElement;
  if (!card) return;
  card.style.setProperty('--tilt-x', '0deg');
  card.style.setProperty('--tilt-y', '0deg');
};
```

- [ ] **Step 3: Update the book card CSS and style binding**

In the CSS block, update `.book-card-inner` transform:
```css
.book-card-inner {
  --tilt-x: 0deg;
  --tilt-y: 0deg;
  transform: rotateX(var(--tilt-x)) rotateY(var(--tilt-y)) translateZ(0px);
  /* existing transition stays */
}
```

In `renderBookCard`, remove the inline `style` binding that references `tiltX()`/`tiltY()`:
```tsx
// Before:
style={{ transform: hoveredCard() === book.id ? `rotateX(${tiltX()}deg)...` : '...' }}
// After: nothing needed — CSS handles it via custom properties
```

- [ ] **Step 4: TypeScript check**

```bash
cd ui && pnpm typecheck 2>&1 | tail -10
```

- [ ] **Step 5: Commit**

```bash
git add ui/src/pages/Reader.tsx
git commit -m "fix(reader): per-card tilt via CSS custom properties — eliminates global tilt signal race on fast mouse moves"
```

---

## Task 14: Fix `chapterCache` anti-pattern

**Files:**
- Modify: `ui/src/pages/Reader.tsx`

- [ ] **Step 1: Replace `createSignal<Map>` with a plain module-level ref**

Find:
```typescript
const [chapterCache] = createSignal<Map<number, string>>(new Map());
```

Replace with a plain `Map` held in a `let` ref — not reactive, mutation is intentional:
```typescript
let chapterCacheMap: Map<number, string> = new Map();
```

- [ ] **Step 2: Update all usages**

```bash
grep -n "chapterCache()" ui/src/pages/Reader.tsx
```

Replace every `chapterCache()` with `chapterCacheMap`. For example:
```typescript
// Before:
const cache = chapterCache();
cache.set(index, chapter.content);
// After:
chapterCacheMap.set(index, chapter.content);

// Before:
const missing = [...new Set(indices)].filter(i => ... && !chapterCache().has(i));
// After:
const missing = [...new Set(indices)].filter(i => ... && !chapterCacheMap.has(i));
```

- [ ] **Step 3: Clear the cache on book open/close**

When opening a new book, reset the cache:
```typescript
chapterCacheMap = new Map();
```

- [ ] **Step 4: TypeScript check**

```bash
cd ui && pnpm typecheck 2>&1 | tail -10
```

- [ ] **Step 5: Commit**

```bash
git add ui/src/pages/Reader.tsx
git commit -m "fix(reader): replace chapterCache createSignal<Map> with plain ref — Map mutations don't need reactivity"
```

---

## Task 15: Deduplicate directory-scan helpers

**Files:**
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: Extract shared `collect_book_files` helper**

Find `collect_files` in `reader_list_folder_files` and `collect_books` in `reader_scan_directory` — they are identical. Before both functions, define once:

```rust
fn collect_book_files(dir: &Path, supported_exts: &[&str], out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_book_files(&path, supported_exts, out);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if supported_exts.contains(&ext.to_lowercase().as_str()) {
                    out.push(path);
                }
            }
        }
    }
}
```

- [ ] **Step 2: Replace the two inline `fn collect_*` definitions**

In `reader_list_folder_files`, remove the local `fn collect_files` and call:
```rust
let mut paths: Vec<PathBuf> = Vec::new();
collect_book_files(&dir, &book_extensions, &mut paths);
let mut candidates: Vec<FolderFileCandidate> = paths.iter().map(|p| FolderFileCandidate {
    path: p.to_string_lossy().to_string(),
    name: p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string(),
    extension: p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase(),
    size: std::fs::metadata(p).map(|m| m.len()).unwrap_or(0),
    already_imported: false,
}).collect();
```

In `reader_scan_directory`, remove local `fn collect_books` and call:
```rust
collect_book_files(&dir, &book_extensions, &mut book_paths);
```

- [ ] **Step 3: Compile**

```bash
cargo build -p minion-app 2>&1 | tail -5
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "refactor(reader): extract shared collect_book_files helper — removes duplicated recursive scan logic"
```

---

## Task 16: Split Reader.tsx into sub-components

**Files:**
- Create: `ui/src/pages/reader/LibraryGrid.tsx`
- Create: `ui/src/pages/reader/EpubReader.tsx`
- Create: `ui/src/pages/reader/PdfReader.tsx`
- Create: `ui/src/pages/reader/CollectionPanel.tsx`
- Modify: `ui/src/pages/Reader.tsx` — becomes a thin coordinator (~300 lines)

- [ ] **Step 1: Create `ui/src/pages/reader/` directory**

```bash
mkdir -p ui/src/pages/reader
```

- [ ] **Step 2: Extract `LibraryGrid.tsx`**

Move out of `Reader.tsx`:
- `renderBookCard` function → becomes the component's render body
- Book card CSS (`.book-card-3d`, `.book-card-inner`, etc.)
- Card tilt handlers (now fixed via CSS in Task 13)
- `addToCollectionBookId` signal and collection dropdown rendering

Props interface:
```typescript
interface LibraryGridProps {
  books: LibraryBook[];
  collections: Collection[];
  loading: boolean;
  openingCardIndex: number | null;
  onOpenBook: (book: LibraryBook, index: number) => void;
  onAddToCollection: (collectionId: string, bookId: string) => void;
  onRemoveFromCollection?: (collectionId: string, bookId: string) => void;
  showRemoveCollectionId?: string;
}
```

- [ ] **Step 3: Extract `EpubReader.tsx`**

Move out of `Reader.tsx`:
- `EpubStPageFlip` component (already extracted — keep as-is)
- `chapterCacheMap` (from Task 14)
- `prefetchEpubChapters`, `fetchChapterIntoCache`, `runEpubPageTurn`, `completeEpubChapterTurn`
- All EPUB-specific signals: `chapterHtml`, `chapterLoading`, `epubTurnOutgoing/IncomingHtml`, `epubTurnDir`, `epubTurnTargetIndex`
- `textScrollContainerRef`, `handleReadScroll`, `scrollTextReadingToTop`
- The reading-mode and font-size controls (shared with PdfReader via props)

Props interface:
```typescript
interface EpubReaderProps {
  book: BookContent;
  bookId: string | null;
  readingMode: ReadingMode;
  fontSize: number;
  onProgressUpdate: (chapterIndex: number) => void;
  onClose: () => void;
}
```

- [ ] **Step 4: Extract `PdfReader.tsx`**

Move out of `Reader.tsx`:
- `pdfDoc`, `pdfCurrentPage`, `pdfTotalPages`, `pdfZoom`, `pdfFitMode`, `pdfLayoutTick`, `pdfLoading`, `pdfPageInputValue`
- `pdfCanvasRef`, `pdfContainerRef`, `pdfResizeObserver`
- `loadPdf`, `renderPdfPage`, `nextPdfPage`, `prevPdfPage`, `runPdfPageChange`, `savePdfProgress`
- `normalizePdfBytes`
- PDF page-swap animation signals/CSS
- `generatePdfThumbnail` (from Task 7)

Props interface:
```typescript
interface PdfReaderProps {
  book: BookContent;
  bookId: string | null;
  onProgressUpdate: (page: number, total: number) => void;
  onClose: () => void;
  onThumbnailSaved?: () => void;
}
```

- [ ] **Step 5: Extract `CollectionPanel.tsx`**

Move out of `Reader.tsx`:
- Collection creation form signals
- `expandedCollection`, `collectionBooks`, `loadingCollectionBooks`
- `addBooksCollectionId`, `addBooksSelected`, `addBooksFilter`
- Folder import modal state: `showImportModal`, `importModalPath`, `importCandidates`, etc.
- Collection-related invoke calls

Props interface:
```typescript
interface CollectionPanelProps {
  collections: Collection[];
  libraryBooks: LibraryBook[];
  onCollectionChange: () => void;  // triggers reload
}
```

- [ ] **Step 6: Update `Reader.tsx` to import and use sub-components**

The slimmed `Reader.tsx` keeps:
- Top-level library state: `libraryBooks`, `collections`, `libraryTab`, `bookSearch`
- `view` signal (`'library' | 'reader'`)
- `currentBook`, `currentBookId`, `currentChapter`
- `loadLibrary`, `loadCollections`, keyboard nav, fullscreen toggle
- `onMount` / `onCleanup`
- Renders `<LibraryGrid>`, `<EpubReader>`, `<PdfReader>`, or `<CollectionPanel>` based on `view` and `libraryTab`

- [ ] **Step 7: TypeScript check (all files)**

```bash
cd ui && pnpm typecheck 2>&1 | tail -20
```
Expected: no errors.

- [ ] **Step 8: Lint**

```bash
cd ui && pnpm lint 2>&1 | tail -10
```

- [ ] **Step 9: Commit**

```bash
git add ui/src/pages/Reader.tsx ui/src/pages/reader/
git commit -m "refactor(reader): split 4004-line Reader.tsx into LibraryGrid, EpubReader, PdfReader, CollectionPanel"
```

---

## Task 17: Final verification

- [ ] **Step 1: Full Rust test suite**

```bash
cargo test --workspace 2>&1 | tail -20
```
Expected: all tests pass.

- [ ] **Step 2: Full TypeScript type check**

```bash
cd ui && pnpm typecheck 2>&1 | tail -10
```

- [ ] **Step 3: Clippy**

```bash
cargo clippy --workspace -- -D warnings 2>&1 | tail -20
```

- [ ] **Step 4: Format check**

```bash
cargo fmt --all -- --check
```

- [ ] **Step 5: Final commit if clean**

```bash
git add -p  # review any remaining changes
git commit -m "chore(reader): final cleanup after audit fixes"
```

---

## Self-Review

**Spec coverage check:**
- B1 (batch import covers) → Task 5 ✓
- B2 (ghost command) → Task 8 ✓
- B3 (unsupported formats) → Task 9 ✓
- B4 (HTML sanitization) → Task 12 ✓
- B5 (global tilt) → Task 13 ✓
- A2 (chapterCache) → Task 14 ✓
- A3 (component split) → Task 16 ✓
- A4 (dedup scan) → Task 15 ✓
- P1 (cover serving) → Tasks 3 + 4 ✓
- P2 (EpubDoc caching) → Deferred ✓
- P3 (batch file_path query) → Task 2 ✓
- P4 (DB index) → Task 1 ✓
- P5 (temp dir cleanup) → Task 11 ✓
- P6 (resource lookup) → Task 10 ✓
- Thumbnails: EPUB batch → Task 5; PDF → Tasks 6 + 7 ✓
- A1, A5 → Deferred ✓

**No placeholders found.**

**Type consistency:** `reader_save_cover` takes `book_id: String, jpeg_bytes: Vec<u8>` on Rust side, `bookId: string, jpegBytes: number[]` on TS side (Tauri serializes `Vec<u8>` as number array). Consistent across Tasks 6 and 7.
