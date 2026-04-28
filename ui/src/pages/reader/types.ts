export interface LibraryBook {
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

export interface Collection {
  id: string;
  name: string;
  description?: string;
  color: string;
  book_count: number;
  created_at: string;
}
