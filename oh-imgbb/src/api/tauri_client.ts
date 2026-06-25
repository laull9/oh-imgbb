import { invoke } from "@tauri-apps/api/core";
import type {
  AlbumDetail,
  AppSettings,
  CachedResponse,
  DetailImageResponse,
  DownloadTaskRecord,
  FavoriteInput,
  FavoriteRecord,
  IbbApiReport,
  IbbCreateAlbumInput,
  IbbEditImageInput,
  LoginStatus,
  ParseTabInput,
  ParseTabRecord,
  ProfileDetail,
} from "./types";

export async function loginImgbb(loginSubject: string, password: string) {
  return invoke<LoginStatus>("login_imgbb", { loginSubject, password });
}

export async function getImgbbLoginStatus() {
  return invoke<LoginStatus>("get_imgbb_login_status");
}

export async function logoutImgbb() {
  return invoke<LoginStatus>("logout_imgbb");
}

export async function createImgbbAlbum(input: IbbCreateAlbumInput) {
  return invoke<IbbApiReport>("create_imgbb_album", { input });
}

export async function uploadImgbbAlbumImage(albumId: string, filePath: string) {
  return invoke<IbbApiReport>("upload_imgbb_album_image", {
    input: { album_id: albumId, file_path: filePath },
  });
}

export async function deleteImgbbImage(imageId: string) {
  return invoke<IbbApiReport>("delete_imgbb_image", { imageId });
}

export async function deleteImgbbAlbum(albumId: string) {
  return invoke<IbbApiReport>("delete_imgbb_album", { albumId });
}

export async function uploadImgbbProfileBackground(filePath: string) {
  return invoke<IbbApiReport>("upload_imgbb_profile_background", {
    input: { file_path: filePath },
  });
}

export async function deleteImgbbProfileBackground() {
  return invoke<IbbApiReport>("delete_imgbb_profile_background");
}

export async function editImgbbImage(input: IbbEditImageInput) {
  return invoke<IbbApiReport>("edit_imgbb_image", { input });
}

export async function parseAlbum(url: string, refresh: boolean) {
  return invoke<CachedResponse<AlbumDetail>>("parse_album", { url, refresh });
}

export async function parseProfile(url: string, refresh: boolean) {
  return invoke<CachedResponse<ProfileDetail>>("parse_profile", {
    url,
    refresh,
  });
}

export async function listParseTabs() {
  return invoke<ParseTabRecord[]>("list_parse_tabs");
}

export async function saveParseTab(tab: ParseTabInput) {
  return invoke<void>("save_parse_tab", { tab });
}

export async function removeParseTab(tabKey: string) {
  return invoke<void>("remove_parse_tab", { tabKey });
}

export async function setActiveParseTab(tabKey?: string) {
  return invoke<void>("set_active_parse_tab", { tabKey });
}

export async function downloadAlbum(url: string) {
  return invoke<DownloadTaskRecord>("download_album", { url });
}

export async function downloadAlbumImages(album: AlbumDetail, imageIds: string[]) {
  return invoke<DownloadTaskRecord>("download_album_images", {
    album,
    imageIds,
  });
}

export async function downloadProfileAlbums(urls: string[]) {
  return invoke<DownloadTaskRecord>("download_profile_albums", { urls });
}

export async function listDownloadTasks() {
  return invoke<DownloadTaskRecord[]>("list_download_tasks");
}

export async function cancelDownloadTask(id: number) {
  return invoke<DownloadTaskRecord>("cancel_download_task", { id });
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

export async function clearThumbnailCache() {
  return invoke<void>("clear_thumbnail_cache");
}

export async function downloadDetailImage(url: string) {
  return invoke<DetailImageResponse>("download_detail_image", { url });
}

export async function removeDetailImage(path: string) {
  return invoke<void>("remove_detail_image", { path });
}
