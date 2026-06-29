import type { AlbumDetail, ProfileAlbum } from "../../api/types";
import type { AlbumTab, ParseTab, ProfileTab } from "./types";
import { normalizeComparableUrl, profileTitle } from "./utils";

// appendProfileAlbums 追加个人空间增量相册。
export function appendProfileAlbums(tabs: ParseTab[], tabKey: string, albums: ProfileAlbum[]): ParseTab[] {
  return tabs.map((tab) => {
    if (tab.kind !== "profile" || tab.key !== tabKey) {
      return tab;
    }

    const known = new Set(tab.albums.map((item) => item.url));
    const next = albums.filter((item) => !known.has(item.url));
    return { ...tab, albums: [...tab.albums, ...next] };
  });
}

// applyAlbumDetail 更新事件返回的相册详情。
export function applyAlbumDetail(tabs: ParseTab[], tabKey: string, album: AlbumDetail): ParseTab[] {
  return tabs.map((tab) =>
    tab.kind === "album" && tab.key === tabKey
      ? {
          ...tab,
          title: album.title,
          url: album.url,
          album,
          selectedImageIds: album.images.map((image) => image.id),
          loading: false,
        }
      : tab,
  );
}

// applyAlbumThumbnail 更新单张图片本地缩略图。
export function applyAlbumThumbnail(
  tabs: ParseTab[],
  tabKey: string,
  imageId: string,
  localThumbnailPath: string,
): ParseTab[] {
  return tabs.map((tab) => {
    if (tab.kind !== "album" || tab.key !== tabKey || !tab.album) {
      return tab;
    }

    return {
      ...tab,
      album: {
        ...tab.album,
        images: tab.album.images.map((image) =>
          image.id === imageId ? { ...image, local_thumbnail_path: localThumbnailPath } : image,
        ),
      },
    };
  });
}

// upsertAlbumTab 打开或重置相册标签。
export function upsertAlbumTab(tabs: ParseTab[], key: string, inputUrl: string, title?: string): ParseTab[] {
  if (tabs.some((tab) => tab.key === key)) {
    return tabs.map((tab) =>
      tab.key === key && tab.kind === "album" ? { ...tab, loading: true } : tab,
    );
  }

  return [
    ...tabs,
    {
      key,
      kind: "album",
      title: title || "相册",
      url: normalizeComparableUrl(inputUrl),
      selectedImageIds: [],
      searchText: "",
      currentPage: 1,
      loading: true,
    } satisfies AlbumTab,
  ];
}

// upsertProfileTab 打开或重置个人空间标签。
export function upsertProfileTab(tabs: ParseTab[], key: string, inputUrl: string, title?: string): ParseTab[] {
  if (tabs.some((tab) => tab.key === key)) {
    return tabs.map((tab) =>
      tab.key === key && tab.kind === "profile"
        ? { ...tab, albums: [], selectedAlbumUrls: [], loading: true }
        : tab,
    );
  }

  return [
    ...tabs,
    {
      key,
      kind: "profile",
      title: title || "个人空间",
      url: normalizeComparableUrl(inputUrl),
      albums: [],
      selectedAlbumUrls: [],
      searchText: "",
      currentPage: 1,
      loading: true,
    } satisfies ProfileTab,
  ];
}

// upsertSearchTab 打开或刷新搜索结果标签。
export function upsertSearchTab(
  tabs: ParseTab[],
  key: string,
  title: string,
  tabUrl: string,
  searchQuery: string,
  albums: ProfileAlbum[],
): ParseTab[] {
  if (tabs.some((tab) => tab.key === key)) {
    return tabs.map((tab) =>
      tab.kind === "profile" && tab.key === key
        ? {
            ...tab,
            title,
            url: tabUrl,
            source: "search",
            searchQuery,
            albums,
            selectedAlbumUrls: [],
            currentPage: 1,
            loading: false,
          }
        : tab,
    );
  }

  return [
    ...tabs,
    {
      key,
      kind: "profile",
      title,
      url: tabUrl,
      source: "search",
      searchQuery,
      albums,
      selectedAlbumUrls: [],
      searchText: "",
      currentPage: 1,
      loading: false,
    } satisfies ProfileTab,
  ];
}

// markTabFailed 标记标签加载失败。
export function markTabFailed(tabs: ParseTab[], key: string): ParseTab[] {
  return tabs.map((tab) =>
    tab.key === key && tab.kind !== "parser" ? { ...tab, loading: false } : tab,
  );
}

// updateAlbumData 写入相册详情数据。
export function updateAlbumData(tabs: ParseTab[], key: string, album: AlbumDetail, loading: boolean): ParseTab[] {
  return tabs.map((tab) =>
    tab.kind === "album" && tab.key === key
      ? {
          ...tab,
          title: album.title,
          url: album.url,
          album,
          selectedImageIds: album.images.map((image) => image.id),
          loading,
        }
      : tab,
  );
}

// updateProfileData 写入个人空间相册数据。
export function updateProfileData(
  tabs: ParseTab[],
  key: string,
  profileUrl: string,
  albums: ProfileAlbum[],
  title: string | undefined,
  loading: boolean,
) {
  return tabs.map((tab) =>
    tab.kind === "profile" && tab.key === key
      ? {
          ...tab,
          title: title || profileTitle(profileUrl),
          url: profileUrl,
          albums,
          selectedAlbumUrls: [],
          loading,
        }
      : tab,
  );
}

// updateAlbumSelection 更新相册图片选择。
export function updateAlbumSelection(tabs: ParseTab[], tabKey: string, selectedImageIds: string[]) {
  return tabs.map((tab) =>
    tab.kind === "album" && tab.key === tabKey ? { ...tab, selectedImageIds } : tab,
  );
}

// updateAlbumSearch 更新相册搜索文本。
export function updateAlbumSearch(tabs: ParseTab[], tabKey: string, searchText: string) {
  return tabs.map((tab) =>
    tab.kind === "album" && tab.key === tabKey ? { ...tab, searchText, currentPage: 1 } : tab,
  );
}

// updateAlbumPage 更新相册当前页。
export function updateAlbumPage(tabs: ParseTab[], tabKey: string, currentPage: number) {
  return tabs.map((tab) =>
    tab.kind === "album" && tab.key === tabKey ? { ...tab, currentPage } : tab,
  );
}

// updateProfileSelection 更新个人空间相册选择。
export function updateProfileSelection(tabs: ParseTab[], tabKey: string, selectedAlbumUrls: string[]) {
  return tabs.map((tab) =>
    tab.kind === "profile" && tab.key === tabKey ? { ...tab, selectedAlbumUrls } : tab,
  );
}

// updateProfileSearch 更新个人空间搜索文本。
export function updateProfileSearch(tabs: ParseTab[], tabKey: string, searchText: string) {
  return tabs.map((tab) =>
    tab.kind === "profile" && tab.key === tabKey ? { ...tab, searchText, currentPage: 1 } : tab,
  );
}

// updateProfilePage 更新个人空间当前页。
export function updateProfilePage(tabs: ParseTab[], tabKey: string, currentPage: number) {
  return tabs.map((tab) =>
    tab.kind === "profile" && tab.key === tabKey ? { ...tab, currentPage } : tab,
  );
}
