import type { AlbumDetail, ProfileAlbum } from "../../api/types";

// ParseKind 标识解析目标类型。
export type ParseKind = "album" | "profile";

// ParseOpenTarget 描述外部页面请求打开的解析目标。
export interface ParseOpenTarget {
  id: number;
  kind: ParseKind;
  url: string;
  title?: string;
}

// ParsePageProps 描述解析页面对外参数。
export interface ParsePageProps {
  openTarget?: ParseOpenTarget;
  onTargetHandled?: (id: number) => void;
  onOpenDownloads?: () => void;
}

// ParserTab 描述固定解析入口标签。
export interface ParserTab {
  key: "parser";
  kind: "parser";
  title: string;
}

// AlbumTab 描述相册解析结果标签。
export interface AlbumTab {
  key: string;
  kind: "album";
  title: string;
  url: string;
  album?: AlbumDetail;
  selectedImageIds: string[];
  searchText: string;
  currentPage: number;
  loading: boolean;
}

// ProfileTab 描述个人空间或搜索结果标签。
export interface ProfileTab {
  key: string;
  kind: "profile";
  title: string;
  url: string;
  source?: "profile" | "search";
  searchQuery?: string;
  albums: ProfileAlbum[];
  selectedAlbumUrls: string[];
  searchText: string;
  currentPage: number;
  loading: boolean;
}

// ParseTab 描述解析页全部标签类型。
export type ParseTab = ParserTab | AlbumTab | ProfileTab;

export const PARSER_TAB_KEY = "parser";

export const DEFAULT_DISPLAY_SETTINGS = {
  pagination_enabled: true,
  profile_page_size: 10,
  album_page_size: 20,
};
