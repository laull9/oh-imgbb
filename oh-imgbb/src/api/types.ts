export interface CachedResponse<T> {
  data: T;
  cached: boolean;
  parsed_at: string;
}

export interface AlbumDetail {
  url: string;
  title: string;
  author_url?: string;
  images: AlbumImage[];
}

export interface AlbumImage {
  id: string;
  filename: string;
  image_url: string;
  thumbnail_url?: string;
  local_thumbnail_path?: string;
  sort_index: number;
}

export interface AlbumThumbnailEvent {
  album_url: string;
  image_id: string;
  thumbnail_url?: string;
  local_thumbnail_path?: string;
  error?: string;
}

export interface DetailImageResponse {
  local_path: string;
}

export interface ProfileDetail {
  url: string;
  albums: ProfileAlbum[];
}

export interface ProfileAlbum {
  name: string;
  url: string;
  cover_url?: string;
}

export interface ProfileBatch {
  page: number;
  albums: ProfileAlbum[];
  finished: boolean;
}

export interface AppSettings {
  download_dir: string;
  max_concurrent_downloads: number;
  max_retries: number;
  file_name_pattern?: string;
  thumbnail_cache_enabled: boolean;
  thumbnail_cache_limit_mb: number;
  restore_last_page: boolean;
  pagination_enabled: boolean;
  profile_page_size: number;
  album_page_size: number;
}

export interface FavoriteRecord {
  id: number;
  kind: string;
  title: string;
  url: string;
  cover_url?: string;
  local_cover_path?: string;
  note?: string;
  created_at: string;
  updated_at: string;
}

export interface FavoriteInput {
  kind: string;
  title: string;
  url: string;
  cover_url?: string;
  note?: string;
}

export interface DownloadReport {
  normalized_url: string;
  author_url?: string;
  directory: string;
  downloaded_files: number;
  bytes_written: number;
}

export interface DownloadBatchReport {
  reports: DownloadReport[];
  downloaded_files: number;
  bytes_written: number;
}

export type DownloadTaskStatus =
  | "pending"
  | "running"
  | "completed"
  | "cancelled"
  | "failed";

export interface DownloadTaskRecord {
  id: number;
  title: string;
  target_kind: string;
  target_url: string;
  status: DownloadTaskStatus;
  total_items: number;
  finished_items: number;
  downloaded_files: number;
  bytes_written: number;
  error_message?: string;
  created_at: string;
  updated_at: string;
}

export interface ParseTabRecord {
  tab_key: string;
  kind: "album" | "profile";
  title: string;
  url: string;
  sort_index: number;
  active: boolean;
  created_at: string;
  updated_at: string;
}

export interface ParseTabInput {
  tab_key: string;
  kind: "album" | "profile";
  title: string;
  url: string;
  sort_index: number;
  active: boolean;
}
