import { invoke } from "@tauri-apps/api/core";
import type {
  AlbumDetail,
  AppSettings,
  CachedResponse,
  DownloadReport,
  FavoriteInput,
  FavoriteRecord,
  ProfileDetail,
} from "./types";

export async function parseAlbum(url: string, refresh: boolean) {
  return invoke<CachedResponse<AlbumDetail>>("parse_album", { url, refresh });
}

export async function parseProfile(url: string, refresh: boolean) {
  return invoke<CachedResponse<ProfileDetail>>("parse_profile", {
    url,
    refresh,
  });
}

export async function downloadAlbum(url: string) {
  return invoke<DownloadReport>("download_album", { url });
}

export async function listFavorites(kind?: string) {
  return invoke<FavoriteRecord[]>("list_favorites", { kind });
}

export async function saveFavorite(favorite: FavoriteInput) {
  return invoke<void>("save_favorite", { favorite });
}

export async function removeFavorite(id: number) {
  return invoke<void>("remove_favorite", { id });
}

export async function getSettings() {
  return invoke<AppSettings>("get_settings");
}

export async function updateSettings(settings: AppSettings) {
  return invoke<AppSettings>("update_settings", { settings });
}
