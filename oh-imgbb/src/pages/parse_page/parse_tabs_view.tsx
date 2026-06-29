import { Tabs } from "antd";
import type { SearchPing } from "../../api/types";
import styles from "../../css/parse_page.module.css";
import * as parseActions from "./actions";
import type { ParseMessage } from "./actions";
import { AlbumTabView } from "./album_tab";
import { ParserTabView } from "./parser_tab";
import { ProfileTabView } from "./profile_tab";
import { ParseTabLabel } from "./tab_label";
import { PARSER_TAB_KEY, type ParseTab } from "./types";

// ParseTabsViewProps 描述解析页标签容器参数。
export interface ParseTabsViewProps {
  tabs: ParseTab[];
  activeKey: string;
  url: string;
  webSearchText: string;
  refresh: boolean;
  parserLoading: boolean;
  webSearchLoading: boolean;
  webSearchPingLoading: boolean;
  webSearchPing?: SearchPing;
  displaySettings: {
    pagination_enabled: boolean;
    profile_page_size: number;
    album_page_size: number;
  };
  message: ParseMessage;
  onOpenDownloads?: () => void;
  onUrlChange: (value: string) => void;
  onWebSearchTextChange: (value: string) => void;
  onRefreshChange: (value: boolean) => void;
  onParse: () => void;
  onWebSearch: () => void;
  onRefreshWebSearchPing: () => void;
  onImportClipboard: () => void;
  onActivateTab: (tabKey: string) => void;
  onCloseTab: (tabKey: string) => void;
  onUpdateAlbumSelection: (tabKey: string, selectedImageIds: string[]) => void;
  onUpdateAlbumSearch: (tabKey: string, searchText: string) => void;
  onUpdateAlbumPage: (tabKey: string, currentPage: number) => void;
  onUpdateProfileSelection: (tabKey: string, selectedAlbumUrls: string[]) => void;
  onUpdateProfileSearch: (tabKey: string, searchText: string) => void;
  onUpdateProfilePage: (tabKey: string, currentPage: number) => void;
  onOpenAlbum: (url: string, refresh: boolean, title?: string) => void;
  onOpenProfile: (url: string, refresh: boolean, title?: string) => void;
}

// ParseTabsView 渲染解析页全部标签。
export function ParseTabsView(props: ParseTabsViewProps) {
  const actionContext = {
    message: props.message,
    onOpenDownloads: props.onOpenDownloads,
  };
  const tabItems = props.tabs.map((tab) => ({
    key: tab.key,
    label: <ParseTabLabel tab={tab} />,
    closable: tab.key !== PARSER_TAB_KEY,
    children:
      tab.kind === "parser" ? (
        <ParserTabView
          url={props.url}
          webSearchText={props.webSearchText}
          refresh={props.refresh}
          parserLoading={props.parserLoading}
          webSearchLoading={props.webSearchLoading}
          webSearchPingLoading={props.webSearchPingLoading}
          webSearchPing={props.webSearchPing}
          onUrlChange={props.onUrlChange}
          onWebSearchTextChange={props.onWebSearchTextChange}
          onRefreshChange={props.onRefreshChange}
          onParse={props.onParse}
          onWebSearch={props.onWebSearch}
          onRefreshWebSearchPing={props.onRefreshWebSearchPing}
          onImportClipboard={props.onImportClipboard}
        />
      ) : tab.kind === "album" ? (
        <AlbumTabView
          tab={tab}
          displaySettings={props.displaySettings}
          onUpdateSelection={props.onUpdateAlbumSelection}
          onFavoriteAlbum={(item) => void parseActions.favoriteAlbum(item, actionContext)}
          onDownloadSelectedImages={(item) => void parseActions.downloadSelectedImages(item, actionContext)}
          onDownloadAlbum={(item) => void parseActions.downloadCurrentAlbum(item, actionContext)}
          onOpenProfile={props.onOpenProfile}
          onUpdateSearch={props.onUpdateAlbumSearch}
          onUpdatePage={props.onUpdateAlbumPage}
        />
      ) : (
        <ProfileTabView
          tab={tab}
          displaySettings={props.displaySettings}
          onUpdateSelection={props.onUpdateProfileSelection}
          onUpdateSearch={props.onUpdateProfileSearch}
          onUpdatePage={props.onUpdateProfilePage}
          onOpenAlbum={props.onOpenAlbum}
          onFavoriteProfile={(item) => void parseActions.favoriteProfile(item, actionContext)}
          onFavoriteProfileAlbum={(item) => void parseActions.favoriteProfileAlbum(item, actionContext)}
          onFavoriteSelectedAlbums={(item) => void parseActions.favoriteSelectedProfileAlbums(item, actionContext)}
          onDownloadSelectedAlbums={(item) => void parseActions.downloadSelectedProfileAlbums(item, actionContext)}
        />
      ),
  }));

  return (
    <Tabs
      type="editable-card"
      hideAdd
      activeKey={props.activeKey}
      onChange={props.onActivateTab}
      onEdit={(targetKey, action) => {
        if (action === "remove") {
          props.onCloseTab(String(targetKey));
        }
      }}
      items={tabItems}
      className={styles.browserTabs}
    />
  );
}
