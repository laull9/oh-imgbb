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
