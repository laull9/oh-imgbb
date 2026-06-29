import {
  downloadAlbum,
  downloadAlbumImages,
  downloadProfileAlbums,
  saveFavorite,
} from "../../api/tauri_client";
import type { ProfileAlbum } from "../../api/types";
import type { AlbumTab, ProfileTab } from "./types";
import { profileTitle } from "./utils";

// ParseMessage 描述解析页动作需要的提示能力。
export interface ParseMessage {
  success: (content: string) => void;
  warning: (content: string) => void;
  error: (content: string) => void;
}

// ParseActionContext 描述解析页动作的外部依赖。
export interface ParseActionContext {
  message: ParseMessage;
  onOpenDownloads?: () => void;
}

// favoriteAlbum 收藏当前相册。
export async function favoriteAlbum(tab: AlbumTab, { message }: ParseActionContext) {
  if (!tab.album) {
    return;
  }

  await saveFavorite({
    kind: "album",
    title: tab.album.title,
    url: tab.album.url,
    cover_url: tab.album.images[0]?.thumbnail_url || tab.album.images[0]?.image_url,
  });
  message.success("已收藏相册");
}

// favoriteProfile 收藏当前个人空间。
export async function favoriteProfile(tab: ProfileTab, { message }: ParseActionContext) {
  await saveFavorite({
    kind: "profile",
    title: profileTitle(tab.url),
    url: tab.url,
    cover_url: tab.albums[0]?.cover_url,
  });
  message.success("已收藏个人空间");
}

// favoriteSelectedProfileAlbums 收藏已选空间相册。
export async function favoriteSelectedProfileAlbums(tab: ProfileTab, { message }: ParseActionContext) {
  const selected = tab.albums.filter((item) => tab.selectedAlbumUrls.includes(item.url));
  if (selected.length === 0) {
    message.warning("请选择要收藏的相册");
    return;
  }

  await Promise.all(
    selected.map((item) =>
      saveFavorite({
        kind: "album",
        title: item.name,
        url: item.url,
        cover_url: item.cover_url,
      }),
    ),
  );
  message.success(`已收藏 ${selected.length} 个相册`);
}

// favoriteProfileAlbum 收藏单个空间相册。
export async function favoriteProfileAlbum(item: ProfileAlbum, { message }: ParseActionContext) {
  try {
    await saveFavorite({
      kind: "album",
      title: item.name,
      url: item.url,
      cover_url: item.cover_url,
    });
    message.success("已收藏相册");
  } catch (error) {
    message.error(String(error));
  }
}

// downloadCurrentAlbum 下载当前相册。
export async function downloadCurrentAlbum(tab: AlbumTab, { message, onOpenDownloads }: ParseActionContext) {
  if (!tab.album) {
    return;
  }

  try {
    await downloadAlbum(tab.album.url);
    message.success("已加入下载任务");
    onOpenDownloads?.();
  } catch (error) {
    message.error(String(error));
  }
}

// downloadSelectedImages 下载已选图片。
export async function downloadSelectedImages(tab: AlbumTab, { message, onOpenDownloads }: ParseActionContext) {
  if (!tab.album || tab.selectedImageIds.length === 0) {
    message.warning("请选择要下载的图片");
    return;
  }

  try {
    await downloadAlbumImages(tab.album, tab.selectedImageIds);
    message.success("已加入下载任务");
    onOpenDownloads?.();
  } catch (error) {
    message.error(String(error));
  }
}

// downloadSelectedProfileAlbums 下载已选空间相册。
export async function downloadSelectedProfileAlbums(
  tab: ProfileTab,
  { message, onOpenDownloads }: ParseActionContext,
) {
  if (tab.selectedAlbumUrls.length === 0) {
    message.warning("请选择要下载的相册");
    return;
  }

  try {
    await downloadProfileAlbums(tab.selectedAlbumUrls);
    message.success("已加入下载任务");
    onOpenDownloads?.();
  } catch (error) {
    message.error(String(error));
  }
}
