import type { AlbumImage, AppSettings, ParseTabRecord, ProfileAlbum } from "../../api/types";
import {
  DEFAULT_DISPLAY_SETTINGS,
  PARSER_TAB_KEY,
  type ParseKind,
  type ParseTab,
} from "./types";

// recordToTab 将持久化记录转换为页面标签。
export function recordToTab(record: ParseTabRecord): ParseTab {
  if (record.kind === "album") {
    return {
      key: record.tab_key,
      kind: "album",
      title: record.title,
      url: record.url,
      selectedImageIds: [],
      searchText: "",
      currentPage: 1,
      loading: true,
    };
  }

  return {
    key: record.tab_key,
    kind: "profile",
    title: record.title,
    url: record.url,
    albums: [],
    selectedAlbumUrls: [],
    searchText: "",
    currentPage: 1,
    loading: true,
  };
}

// settingsToDisplaySettings 转换解析页展示设置。
export function settingsToDisplaySettings(settings: AppSettings) {
  return {
    pagination_enabled: settings.pagination_enabled,
    profile_page_size: Math.max(1, settings.profile_page_size || DEFAULT_DISPLAY_SETTINGS.profile_page_size),
    album_page_size: Math.max(1, settings.album_page_size || DEFAULT_DISPLAY_SETTINGS.album_page_size),
  };
}

// filterAlbumImages 按文件名过滤相册图片。
export function filterAlbumImages(images: AlbumImage[], searchText: string) {
  const keyword = normalizeSearchText(searchText);
  if (!keyword) {
    return images;
  }

  return images.filter((image) => normalizeSearchText(image.filename).includes(keyword));
}

// filterProfileAlbums 按相册名称过滤空间相册。
export function filterProfileAlbums(albums: ProfileAlbum[], searchText: string) {
  const keyword = normalizeSearchText(searchText);
  if (!keyword) {
    return albums;
  }

  return albums.filter((album) => normalizeSearchText(album.name).includes(keyword));
}

// normalizeSearchText 规整搜索文本用于比较。
export function normalizeSearchText(value: string) {
  return value.trim().toLocaleLowerCase();
}

// paginateList 截取当前页数据。
export function paginateList<T>(items: T[], currentPage: number, pageSize: number) {
  const safePageSize = Math.max(1, pageSize);
  const safePage = clampPage(currentPage, items.length, safePageSize);
  const start = (safePage - 1) * safePageSize;

  return items.slice(start, start + safePageSize);
}

// clampPage 将页码限制在合法范围内。
export function clampPage(currentPage: number, total: number, pageSize: number) {
  const safePageSize = Math.max(1, pageSize);
  const maxPage = Math.max(1, Math.ceil(total / safePageSize));

  return Math.min(Math.max(1, currentPage), maxPage);
}

// mergeSelection 合并选择项并去重。
export function mergeSelection(current: string[], next: string[]) {
  return Array.from(new Set([...current, ...next]));
}

// removeSelection 从当前选择项中移除指定项。
export function removeSelection(current: string[], removed: string[]) {
  const removedSet = new Set(removed);

  return current.filter((item) => !removedSet.has(item));
}

// buildTabKey 构造解析结果标签 key。
export function buildTabKey(kind: ParseKind, inputUrl: string) {
  return `${kind}:${normalizeComparableUrl(inputUrl)}`;
}

// buildSearchTabKey 构造搜索结果标签 key。
export function buildSearchTabKey(query: string) {
  return `search:${normalizeSearchText(query)}`;
}

// detectParseKind 根据 URL 形态自动判断解析目标类型。
export function detectParseKind(inputUrl: string): ParseKind | undefined {
  const value = inputUrl.trim();
  const withScheme = /^https?:\/\//i.test(value) ? value : `https://${value}`;

  try {
    const parsed = new URL(withScheme);
    const host = parsed.hostname.toLowerCase();
    const pathParts = parsed.pathname.split("/").filter(Boolean);

    if ((host === "ibb.co" || host === "www.ibb.co") && pathParts[0] === "album" && pathParts[1]) {
      return "album";
    }

    if (host.endsWith(".imgbb.com")) {
      return "profile";
    }
  } catch {
    return undefined;
  }

  return undefined;
}

// normalizeComparableUrl 规整 URL 以便复用标签和缓存。
export function normalizeComparableUrl(inputUrl: string) {
  const value = inputUrl.trim();
  const withScheme = /^https?:\/\//i.test(value) ? value : `https://${value}`;

  try {
    const parsed = new URL(withScheme);
    const host = parsed.hostname.toLowerCase();
    const pathParts = parsed.pathname.split("/").filter(Boolean);

    if ((host === "ibb.co" || host === "www.ibb.co") && pathParts[0] === "album" && pathParts[1]) {
      return `https://ibb.co/album/${pathParts[1]}/`;
    }

    if (host.endsWith(".imgbb.com")) {
      const sort = parsed.searchParams.get("sort");
      const normalized = new URL(`${parsed.protocol}//${host}/albums`);
      normalized.searchParams.set("list", "albums");
      if (sort) {
        normalized.searchParams.set("sort", sort);
      }
      return normalized.toString();
    }
  } catch {
    return value;
  }

  return withScheme;
}

// profileTitle 从个人空间 URL 提取展示标题。
export function profileTitle(profileUrl: string) {
  try {
    return new URL(profileUrl).hostname;
  } catch {
    return "个人空间";
  }
}

// truncateText 截断过长标签标题。
export function truncateText(value: string, maxLength: number) {
  if (value.length <= maxLength) {
    return value;
  }

  return `${value.slice(0, maxLength)}...`;
}

// createParserTab 创建固定解析入口标签。
export function createParserTab(): ParseTab {
  return { key: PARSER_TAB_KEY, kind: "parser", title: "解析" };
}
