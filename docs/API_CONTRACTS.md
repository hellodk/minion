# MINION API Contracts

## Overview

MINION exposes two API layers:
1. **Internal IPC API**: Tauri commands for frontend-backend communication
2. **Plugin API**: Interface for external plugins to interact with core

---

## IPC API (Tauri Commands)

### Core Commands

```typescript
// ============================================================
// SYSTEM COMMANDS
// ============================================================

interface SystemInfo {
  version: string;
  buildDate: string;
  platform: 'linux' | 'windows' | 'macos';
  arch: 'x64' | 'arm64';
  dataDir: string;
  configDir: string;
}

// Get system information
#[tauri::command]
async fn get_system_info() -> Result<SystemInfo, Error>;

// Get application config
#[tauri::command]
async fn get_config(key: Option<String>) -> Result<Value, Error>;

// Set application config
#[tauri::command]
async fn set_config(key: String, value: Value) -> Result<(), Error>;

// ============================================================
// MODULE MANAGEMENT
// ============================================================

interface ModuleInfo {
  id: string;
  name: string;
  version: string;
  enabled: boolean;
  permissions: string[];
  status: 'active' | 'inactive' | 'error';
}

// List all modules
#[tauri::command]
async fn list_modules() -> Result<Vec<ModuleInfo>, Error>;

// Enable/disable module
#[tauri::command]
async fn set_module_enabled(module_id: String, enabled: bool) -> Result<(), Error>;

// Get module status
#[tauri::command]
async fn get_module_status(module_id: String) -> Result<ModuleStatus, Error>;

// ============================================================
// CREDENTIAL VAULT
// ============================================================

// Store credential (encrypted)
#[tauri::command]
async fn store_credential(service: String, credential: CredentialInput) -> Result<(), Error>;

// Retrieve credential
#[tauri::command]
async fn get_credential(service: String) -> Result<Option<Credential>, Error>;

// Delete credential
#[tauri::command]
async fn delete_credential(service: String) -> Result<(), Error>;

// List services with stored credentials
#[tauri::command]
async fn list_credential_services() -> Result<Vec<String>, Error>;

// ============================================================
// BACKGROUND TASKS
// ============================================================

interface TaskInfo {
  id: string;
  moduleId: string;
  taskType: string;
  status: 'pending' | 'running' | 'completed' | 'failed';
  progress: number;  // 0-100
  createdAt: string;
  startedAt?: string;
  completedAt?: string;
  error?: string;
}

// Get task status
#[tauri::command]
async fn get_task_status(task_id: String) -> Result<TaskInfo, Error>;

// List active tasks
#[tauri::command]
async fn list_tasks(filter: Option<TaskFilter>) -> Result<Vec<TaskInfo>, Error>;

// Cancel task
#[tauri::command]
async fn cancel_task(task_id: String) -> Result<(), Error>;

// ============================================================
// GLOBAL SEARCH
// ============================================================

interface SearchResult {
  module: string;
  type: string;
  id: string;
  title: string;
  snippet: string;
  score: number;
  metadata: Record<string, any>;
}

// Global search across all modules
#[tauri::command]
async fn global_search(
  query: String,
  modules: Option<Vec<String>>,
  limit: Option<u32>
) -> Result<Vec<SearchResult>, Error>;

// Semantic search (RAG)
#[tauri::command]
async fn semantic_search(
  query: String,
  modules: Option<Vec<String>>,
  limit: Option<u32>
) -> Result<Vec<SearchResult>, Error>;
```

### Module 1: Media Intelligence Commands

```typescript
// ============================================================
// MEDIA FILES
// ============================================================

interface MediaFile {
  id: string;
  filePath: string;
  fileName: string;
  fileSize: number;
  mimeType: string;
  duration?: number;
  width?: number;
  height?: number;
  thumbnailPath?: string;
  metadata: Record<string, any>;
}

// Import media file
#[tauri::command]
async fn media_import_file(path: String) -> Result<MediaFile, Error>;

// List media files
#[tauri::command]
async fn media_list_files(filter: MediaFilter) -> Result<PaginatedResult<MediaFile>, Error>;

// Get media file details
#[tauri::command]
async fn media_get_file(id: String) -> Result<MediaFile, Error>;

// Delete media file
#[tauri::command]
async fn media_delete_file(id: String) -> Result<(), Error>;

// ============================================================
// YOUTUBE INTEGRATION
// ============================================================

interface YouTubeAccount {
  id: string;
  channelId: string;
  channelName: string;
  email: string;
  isDefault: boolean;
  quotaUsed: number;
}

// OAuth flow
#[tauri::command]
async fn youtube_start_oauth() -> Result<String, Error>;  // Returns auth URL

#[tauri::command]
async fn youtube_complete_oauth(code: String) -> Result<YouTubeAccount, Error>;

// Account management
#[tauri::command]
async fn youtube_list_accounts() -> Result<Vec<YouTubeAccount>, Error>;

#[tauri::command]
async fn youtube_remove_account(account_id: String) -> Result<(), Error>;

#[tauri::command]
async fn youtube_set_default_account(account_id: String) -> Result<(), Error>;

// ============================================================
// UPLOAD MANAGEMENT
// ============================================================

interface UploadDraft {
  mediaFileId: string;
  accountId: string;
  title: string;
  description: string;
  tags: string[];
  categoryId: number;
  privacyStatus: 'public' | 'unlisted' | 'private';
  playlistIds?: string[];
  scheduledAt?: string;
  thumbnailPath?: string;
}

interface Upload {
  id: string;
  status: 'draft' | 'queued' | 'uploading' | 'processing' | 'published' | 'failed';
  progress: number;
  youtubeVideoId?: string;
  error?: string;
  // ... all draft fields
}

// Create upload draft
#[tauri::command]
async fn youtube_create_draft(draft: UploadDraft) -> Result<Upload, Error>;

// Update upload
#[tauri::command]
async fn youtube_update_upload(id: String, updates: PartialUploadDraft) -> Result<Upload, Error>;

// Queue upload
#[tauri::command]
async fn youtube_queue_upload(id: String) -> Result<Upload, Error>;

// Get upload status
#[tauri::command]
async fn youtube_get_upload(id: String) -> Result<Upload, Error>;

// List uploads
#[tauri::command]
async fn youtube_list_uploads(filter: UploadFilter) -> Result<PaginatedResult<Upload>, Error>;

// Cancel upload
#[tauri::command]
async fn youtube_cancel_upload(id: String) -> Result<(), Error>;

// Retry failed upload
#[tauri::command]
async fn youtube_retry_upload(id: String) -> Result<Upload, Error>;

// ============================================================
// AI GENERATION
// ============================================================

interface AIGeneratedContent {
  titles: string[];
  descriptions: string[];
  tags: string[];
  thumbnailSuggestions: string[];
}

// Generate AI content for video
#[tauri::command]
async fn media_ai_generate(
  media_file_id: String,
  options: AIGenerationOptions
) -> Result<AIGeneratedContent, Error>;

// Generate thumbnail
#[tauri::command]
async fn media_generate_thumbnail(
  media_file_id: String,
  options: ThumbnailOptions
) -> Result<String, Error>;  // Returns thumbnail path
```

### Module 2: File Intelligence Commands

```typescript
// ============================================================
// SCANNING
// ============================================================

interface ScanDirectory {
  id: string;
  path: string;
  recursive: boolean;
  includePatterns: string[];
  excludePatterns: string[];
  lastScanAt?: string;
  fileCount: number;
  totalSize: number;
  enabled: boolean;
}

// Add directory to scan
#[tauri::command]
async fn files_add_directory(config: ScanDirectoryConfig) -> Result<ScanDirectory, Error>;

// Remove directory
#[tauri::command]
async fn files_remove_directory(id: String) -> Result<(), Error>;

// Start scan
#[tauri::command]
async fn files_start_scan(directory_id: Option<String>) -> Result<String, Error>;  // Returns task ID

// Get scan progress
#[tauri::command]
async fn files_get_scan_progress(task_id: String) -> Result<ScanProgress, Error>;

// ============================================================
// DUPLICATE DETECTION
// ============================================================

interface DuplicateGroup {
  id: string;
  matchType: 'exact' | 'perceptual' | 'audio' | 'near';
  similarityScore: number;
  fileCount: number;
  totalSize: number;
  wastedSpace: number;
  files: FileInfo[];
  status: 'pending' | 'reviewed' | 'resolved';
}

// Find duplicates
#[tauri::command]
async fn files_find_duplicates(options: DuplicateOptions) -> Result<String, Error>;  // Returns task ID

// List duplicate groups
#[tauri::command]
async fn files_list_duplicates(filter: DuplicateFilter) -> Result<PaginatedResult<DuplicateGroup>, Error>;

// Resolve duplicate group
#[tauri::command]
async fn files_resolve_duplicates(
  group_id: String,
  keep_file_ids: Vec<String>,
  action: 'delete' | 'move' | 'ignore'
) -> Result<(), Error>;

// ============================================================
// ANALYTICS
// ============================================================

interface StorageAnalytics {
  totalFiles: number;
  totalSize: number;
  byExtension: Record<string, { count: number; size: number }>;
  byAge: Record<string, { count: number; size: number }>;
  duplicatesFound: number;
  duplicateSize: number;
  largestFiles: FileInfo[];
  oldestFiles: FileInfo[];
}

// Get storage analytics
#[tauri::command]
async fn files_get_analytics(directory_id: Option<String>) -> Result<StorageAnalytics, Error>;

// Get folder heatmap
#[tauri::command]
async fn files_get_heatmap(path: String, depth: u32) -> Result<Vec<HeatmapNode>, Error>;
```

### Module 3: Blog AI Commands

```typescript
// ============================================================
// BLOG POSTS
// ============================================================

interface BlogPost {
  id: string;
  title: string;
  slug: string;
  contentMarkdown: string;
  contentHtml: string;
  excerpt: string;
  metaTitle: string;
  metaDescription: string;
  keywords: string[];
  seoScore: number;
  category: string;
  tags: string[];
  status: 'draft' | 'scheduled' | 'published';
  createdAt: string;
  updatedAt: string;
  scheduledAt?: string;
  publishedAt?: string;
}

// CRUD operations
#[tauri::command]
async fn blog_create_post(post: NewBlogPost) -> Result<BlogPost, Error>;

#[tauri::command]
async fn blog_update_post(id: String, updates: PartialBlogPost) -> Result<BlogPost, Error>;

#[tauri::command]
async fn blog_get_post(id: String) -> Result<BlogPost, Error>;

#[tauri::command]
async fn blog_list_posts(filter: BlogPostFilter) -> Result<PaginatedResult<BlogPost>, Error>;

#[tauri::command]
async fn blog_delete_post(id: String) -> Result<(), Error>;

// ============================================================
// PLATFORMS
// ============================================================

interface BlogPlatform {
  id: string;
  platformType: 'wordpress' | 'medium' | 'hashnode' | 'devto' | 'custom';
  name: string;
  apiEndpoint: string;
  isDefault: boolean;
}

#[tauri::command]
async fn blog_add_platform(config: PlatformConfig) -> Result<BlogPlatform, Error>;

#[tauri::command]
async fn blog_list_platforms() -> Result<Vec<BlogPlatform>, Error>;

#[tauri::command]
async fn blog_remove_platform(id: String) -> Result<(), Error>;

// ============================================================
// PUBLISHING
// ============================================================

#[tauri::command]
async fn blog_publish(
  post_id: String,
  platform_ids: Vec<String>,
  schedule_at: Option<String>
) -> Result<Vec<PublicationStatus>, Error>;

#[tauri::command]
async fn blog_get_publication_status(post_id: String) -> Result<Vec<PublicationStatus>, Error>;

// ============================================================
// AI FEATURES
// ============================================================

interface AIBlogSuggestions {
  titles: string[];
  keywords: string[];
  tags: string[];
  outline: string[];
  internalLinks: string[];
}

#[tauri::command]
async fn blog_ai_suggest(content: String, options: AISuggestOptions) -> Result<AIBlogSuggestions, Error>;

#[tauri::command]
async fn blog_ai_generate_content(topic: String, options: ContentGenOptions) -> Result<string, Error>;

#[tauri::command]
async fn blog_analyze_seo(content: String, keywords: Vec<String>) -> Result<SEOAnalysis, Error>;
```

### Module 4: Finance Commands

```typescript
// ============================================================
// ACCOUNTS
// ============================================================

interface FinanceAccount {
  id: string;
  name: string;
  accountType: 'bank' | 'credit_card' | 'investment' | 'loan' | 'wallet';
  institution: string;
  currency: string;
  currentBalance: number;
  isActive: boolean;
}

#[tauri::command]
async fn finance_create_account(account: NewAccount) -> Result<FinanceAccount, Error>;

#[tauri::command]
async fn finance_list_accounts() -> Result<Vec<FinanceAccount>, Error>;

#[tauri::command]
async fn finance_update_account(id: String, updates: PartialAccount) -> Result<FinanceAccount, Error>;

// ============================================================
// TRANSACTIONS
// ============================================================

interface Transaction {
  id: string;
  accountId: string;
  date: string;
  description: string;
  amount: number;
  transactionType: 'credit' | 'debit';
  category: string;
  subcategory: string;
  tags: string[];
}

#[tauri::command]
async fn finance_add_transaction(txn: NewTransaction) -> Result<Transaction, Error>;

#[tauri::command]
async fn finance_list_transactions(filter: TransactionFilter) -> Result<PaginatedResult<Transaction>, Error>;

#[tauri::command]
async fn finance_import_statement(
  file_path: String,
  account_id: String,
  format: 'csv' | 'pdf'
) -> Result<ImportResult, Error>;

#[tauri::command]
async fn finance_categorize_transaction(id: String, category: String) -> Result<(), Error>;

// AI categorization
#[tauri::command]
async fn finance_ai_categorize(transaction_ids: Vec<String>) -> Result<Vec<CategorySuggestion>, Error>;

// ============================================================
// INVESTMENTS
// ============================================================

interface Holding {
  id: string;
  accountId: string;
  symbol: string;
  name: string;
  holdingType: 'stock' | 'mutual_fund' | 'etf' | 'bond' | 'crypto';
  quantity: number;
  avgBuyPrice: number;
  currentPrice: number;
  currentValue: number;
  totalGainLoss: number;
  totalGainLossPercent: number;
}

#[tauri::command]
async fn finance_list_holdings(account_id: Option<String>) -> Result<Vec<Holding>, Error>;

#[tauri::command]
async fn finance_add_holding(holding: NewHolding) -> Result<Holding, Error>;

#[tauri::command]
async fn finance_record_trade(trade: TradeRecord) -> Result<(), Error>;

#[tauri::command]
async fn finance_refresh_prices() -> Result<(), Error>;

// ============================================================
// ANALYTICS
// ============================================================

interface FinancialSummary {
  netWorth: number;
  totalAssets: number;
  totalLiabilities: number;
  monthlyIncome: number;
  monthlyExpenses: number;
  savingsRate: number;
  topExpenseCategories: CategorySummary[];
  investmentReturns: number;
}

#[tauri::command]
async fn finance_get_summary(period: string) -> Result<FinancialSummary, Error>;

#[tauri::command]
async fn finance_get_spending_trends(months: u32) -> Result<SpendingTrends, Error>;

#[tauri::command]
async fn finance_calculate_fire(params: FIREParams) -> Result<FIREProjection, Error>;

#[tauri::command]
async fn finance_estimate_tax(year: u32) -> Result<TaxEstimate, Error>;
```

### Module 5: Fitness Commands

```typescript
// ============================================================
// PROFILE & METRICS
// ============================================================

#[tauri::command]
async fn fitness_get_profile() -> Result<FitnessProfile, Error>;

#[tauri::command]
async fn fitness_update_profile(updates: PartialProfile) -> Result<FitnessProfile, Error>;

#[tauri::command]
async fn fitness_log_weight(entry: WeightEntry) -> Result<(), Error>;

#[tauri::command]
async fn fitness_get_weight_history(days: u32) -> Result<Vec<WeightEntry>, Error>;

// ============================================================
// WORKOUTS
// ============================================================

#[tauri::command]
async fn fitness_create_workout(workout: NewWorkout) -> Result<Workout, Error>;

#[tauri::command]
async fn fitness_list_workouts(filter: WorkoutFilter) -> Result<PaginatedResult<Workout>, Error>;

#[tauri::command]
async fn fitness_get_workout_stats(period: string) -> Result<WorkoutStats, Error>;

// ============================================================
// HABITS
// ============================================================

#[tauri::command]
async fn fitness_create_habit(habit: NewHabit) -> Result<Habit, Error>;

#[tauri::command]
async fn fitness_list_habits() -> Result<Vec<Habit>, Error>;

#[tauri::command]
async fn fitness_log_habit(habit_id: String, date: String, completed: bool) -> Result<(), Error>;

#[tauri::command]
async fn fitness_get_habit_streaks() -> Result<Vec<HabitStreak>, Error>;

// ============================================================
// NUTRITION
// ============================================================

#[tauri::command]
async fn fitness_log_meal(entry: MealEntry) -> Result<(), Error>;

#[tauri::command]
async fn fitness_get_nutrition_summary(date: String) -> Result<NutritionSummary, Error>;

// ============================================================
// 75 HARD
// ============================================================

#[tauri::command]
async fn fitness_start_75hard(start_date: String) -> Result<(), Error>;

#[tauri::command]
async fn fitness_log_75hard(entry: Day75HardEntry) -> Result<(), Error>;

#[tauri::command]
async fn fitness_get_75hard_progress() -> Result<Challenge75HardProgress, Error>;
```

### Module 6: Book Reader Commands

```typescript
// ============================================================
// LIBRARY
// ============================================================

interface Book {
  id: string;
  filePath: string;
  fileFormat: string;
  title: string;
  subtitle?: string;
  authors: string[];
  publisher?: string;
  publishDate?: string;
  isbn?: string;
  coverPath?: string;
  totalPages: number;
  estimatedHours: number;
  dateAdded: string;
  lastOpened?: string;
  isFavorite: boolean;
  rating?: number;
  collections: string[];
  tags: string[];
}

#[tauri::command]
async fn reader_import_book(path: String) -> Result<Book, Error>;

#[tauri::command]
async fn reader_import_directory(path: String) -> Result<ImportResult, Error>;

#[tauri::command]
async fn reader_list_books(filter: BookFilter) -> Result<PaginatedResult<Book>, Error>;

#[tauri::command]
async fn reader_get_book(id: String) -> Result<Book, Error>;

#[tauri::command]
async fn reader_delete_book(id: String, delete_file: bool) -> Result<(), Error>;

#[tauri::command]
async fn reader_update_book(id: String, updates: PartialBook) -> Result<Book, Error>;

// ============================================================
// READING
// ============================================================

interface BookContent {
  chapters: Chapter[];
  currentChapter: number;
  content: string;  // HTML content for current view
  position: ReadingPosition;
}

#[tauri::command]
async fn reader_open_book(id: String) -> Result<BookContent, Error>;

#[tauri::command]
async fn reader_get_chapter(book_id: String, chapter_index: u32) -> Result<ChapterContent, Error>;

#[tauri::command]
async fn reader_update_position(book_id: String, position: ReadingPosition) -> Result<(), Error>;

#[tauri::command]
async fn reader_get_progress(book_id: String) -> Result<ReadingProgress, Error>;

// ============================================================
// ANNOTATIONS
// ============================================================

interface Annotation {
  id: string;
  bookId: string;
  chapterIndex: number;
  highlightedText: string;
  annotationText?: string;
  annotationType: 'highlight' | 'note' | 'bookmark';
  color: string;
  createdAt: string;
}

#[tauri::command]
async fn reader_create_annotation(annotation: NewAnnotation) -> Result<Annotation, Error>;

#[tauri::command]
async fn reader_list_annotations(book_id: String) -> Result<Vec<Annotation>, Error>;

#[tauri::command]
async fn reader_delete_annotation(id: String) -> Result<(), Error>;

#[tauri::command]
async fn reader_export_annotations(book_id: String, format: 'md' | 'json') -> Result<String, Error>;

// ============================================================
// AI FEATURES
// ============================================================

#[tauri::command]
async fn reader_ai_summarize_chapter(book_id: String, chapter_index: u32) -> Result<String, Error>;

#[tauri::command]
async fn reader_ai_ask(query: String, book_ids: Option<Vec<String>>) -> Result<AIAnswer, Error>;

#[tauri::command]
async fn reader_ai_concept_map(book_id: String) -> Result<ConceptMap, Error>;

// ============================================================
// READING STATS
// ============================================================

#[tauri::command]
async fn reader_start_session(book_id: String) -> Result<String, Error>;  // Returns session ID

#[tauri::command]
async fn reader_end_session(session_id: String) -> Result<SessionSummary, Error>;

#[tauri::command]
async fn reader_get_stats(period: string) -> Result<ReadingStats, Error>;

#[tauri::command]
async fn reader_get_goals() -> Result<Vec<ReadingGoal>, Error>;

#[tauri::command]
async fn reader_set_goal(goal: NewReadingGoal) -> Result<ReadingGoal, Error>;

// ============================================================
// COLLECTIONS
// ============================================================

#[tauri::command]
async fn reader_create_collection(name: String) -> Result<Collection, Error>;

#[tauri::command]
async fn reader_list_collections() -> Result<Vec<Collection>, Error>;

#[tauri::command]
async fn reader_add_to_collection(book_id: String, collection_id: String) -> Result<(), Error>;
```

---

## Event Types (Frontend Subscriptions)

```typescript
// Events emitted from backend to frontend

interface MinionEvent {
  type: string;
  payload: any;
  timestamp: string;
}

// System events
type SystemEvent = 
  | { type: 'module:loaded'; payload: { moduleId: string } }
  | { type: 'module:error'; payload: { moduleId: string; error: string } }
  | { type: 'task:progress'; payload: { taskId: string; progress: number; message: string } }
  | { type: 'task:completed'; payload: { taskId: string; result: any } }
  | { type: 'task:failed'; payload: { taskId: string; error: string } };

// Media events
type MediaEvent =
  | { type: 'media:import:progress'; payload: { fileId: string; progress: number } }
  | { type: 'media:upload:progress'; payload: { uploadId: string; progress: number } }
  | { type: 'media:upload:completed'; payload: { uploadId: string; videoId: string } };

// File events
type FileEvent =
  | { type: 'files:scan:progress'; payload: { scanned: number; total: number } }
  | { type: 'files:duplicates:found'; payload: { groupId: string } };

// Reader events
type ReaderEvent =
  | { type: 'reader:import:progress'; payload: { current: number; total: number } }
  | { type: 'reader:ai:ready'; payload: { bookId: string } };

// Subscribe to events (frontend)
listen<MinionEvent>('minion-event', (event) => {
  // Handle event
});
```

---

## Error Handling

```typescript
interface MinionError {
  code: string;
  message: string;
  details?: Record<string, any>;
  recoverable: boolean;
  suggestion?: string;
}

// Error codes
enum ErrorCode {
  // System
  SYSTEM_ERROR = 'E0001',
  MODULE_NOT_FOUND = 'E0002',
  MODULE_DISABLED = 'E0003',
  
  // Auth
  AUTH_REQUIRED = 'E1001',
  AUTH_EXPIRED = 'E1002',
  AUTH_INVALID = 'E1003',
  
  // Data
  NOT_FOUND = 'E2001',
  VALIDATION_ERROR = 'E2002',
  DUPLICATE_ENTRY = 'E2003',
  
  // External
  API_ERROR = 'E3001',
  RATE_LIMITED = 'E3002',
  NETWORK_ERROR = 'E3003',
  
  // File
  FILE_NOT_FOUND = 'E4001',
  FILE_ACCESS_DENIED = 'E4002',
  FILE_TOO_LARGE = 'E4003',
  UNSUPPORTED_FORMAT = 'E4004',
}
```

---

## Rate Limiting & Quotas

```typescript
interface QuotaInfo {
  service: string;
  used: number;
  limit: number;
  resetsAt: string;
}

// Check quota before operations
#[tauri::command]
async fn check_quota(service: String) -> Result<QuotaInfo, Error>;

// Services with quotas:
// - youtube: 10,000 units/day
// - openai: varies by plan
// - medium: 100 posts/day
```
