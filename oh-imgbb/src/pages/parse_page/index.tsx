import { App } from "antd";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import {
  getSettings,
  listParseTabs,
  parseAlbum,
  parseProfile,
  pingWebsearch,
  removeParseTab,
  saveParseTab,
  searchImgbbAlbums,
  setActiveParseTab,
} from "../../api/tauri_client";
import type { AlbumDetail, AlbumThumbnailEvent, CachedResponse, ParseTabRecord, ProfileAlbum, ProfileBatch } from "../../api/types";
import { useAppStore } from "../../tools/store";
import { ParseTabsView } from "./parse_tabs_view";
import * as tabState from "./tab_state";
import { DEFAULT_DISPLAY_SETTINGS, PARSER_TAB_KEY, type ParseKind, type ParsePageProps, type ParseTab } from "./types";
import {
  buildSearchTabKey, buildTabKey, createParserTab, detectParseKind, normalizeComparableUrl,
  profileTitle, recordToTab, settingsToDisplaySettings,
} from "./utils";

export type { ParseOpenTarget } from "./types";

// ParsePage 展示 ImgBB 地址解析、搜索和结果标签。
export function ParsePage({ openTarget, onTargetHandled, onOpenDownloads }: ParsePageProps) {
  const { message } = App.useApp();
  const setAppState = useAppStore((state) => state.setState);
  const webSearchText = useAppStore((state) => state.webSearchText);
  const webSearchPing = useAppStore((state) => state.webSearchPing);
  const webSearchPingLoading = useAppStore((state) => state.webSearchPingLoading);
  const loadedWebSearchDetect = useAppStore((state) => state.loadedWebSearchDetect);
  const [url, setUrl] = useState("");
  const [refresh, setRefresh] = useState(false);
  const [parserLoading, setParserLoading] = useState(false);
  const [webSearchLoading, setWebSearchLoading] = useState(false);
  const [activeKey, setActiveKey] = useState(PARSER_TAB_KEY);
  const [tabs, setTabs] = useState<ParseTab[]>([createParserTab()]);
  const [displaySettings, setDisplaySettings] = useState(DEFAULT_DISPLAY_SETTINGS);
  const [tabsRestored, setTabsRestored] = useState(false);
  const pendingProfileTabKey = useRef<string | undefined>(undefined);
  const tabHistoryRef = useRef<string[]>([PARSER_TAB_KEY]);

  useEffect(() => {
    let disposed = false;

    getSettings()
      .then((settings) => {
        if (!disposed) {
          setDisplaySettings(settingsToDisplaySettings(settings));
        }
      })
      .catch((error) => {
        if (!disposed) {
          message.error(String(error));
        }
      });

    return () => {
      disposed = true;
    };
  }, [message]);

  useEffect(() => {
    if (!loadedWebSearchDetect) {
      void refreshWebSearchPing();
    }
  }, [loadedWebSearchDetect]);

  useEffect(() => {
    let disposed = false;

    async function restoreTabs() {
      try {
        const records = await listParseTabs();
        if (disposed || records.length === 0) {
          return;
        }

        const restoredActiveKey = records.find((record) => record.active)?.tab_key || PARSER_TAB_KEY;

        setTabs([createParserTab(), ...records.map(recordToTab)]);
        setActiveKey(restoredActiveKey);
        rememberTab(restoredActiveKey);
        setTabsRestored(true);

        await Promise.all(records.map((record) => loadRestoredTab(record)));
      } catch (error) {
        if (!disposed) {
          message.error(String(error));
        }
      } finally {
        if (!disposed) {
          setTabsRestored(true);
        }
      }
    }

    async function loadRestoredTab(record: ParseTabRecord) {
      try {
        if (record.kind === "album") {
          const response = await parseAlbum(record.url, false);
          if (!disposed) {
            updateAlbumTab(record.tab_key, response.data, false);
          }
          return;
        }

        const response = await parseProfile(record.url, false);
        if (!disposed) {
          updateProfileTab(record.tab_key, response.data.url, response.data.albums, record.title, false);
        }
      } catch {
        if (!disposed) {
          markTabFailed(record.tab_key);
        }
      }
    }

    void restoreTabs();

    return () => {
      disposed = true;
    };
  }, [message]);

  useEffect(() => {
    let disposed = false;
    let unlistenProfile: (() => void) | undefined;
    let unlistenAlbumDetail: (() => void) | undefined;
    let unlistenAlbumThumbnail: (() => void) | undefined;

    listen<ProfileBatch>("profile://album_found", (event) => {
      const tabKey = pendingProfileTabKey.current;
      if (disposed || event.payload.finished || !tabKey) {
        return;
      }

      setTabs((current) => tabState.appendProfileAlbums(current, tabKey, event.payload.albums));
    }).then((value) => {
      unlistenProfile = value;
    });

    listen<CachedResponse<AlbumDetail>>("album://detail_ready", (event) => {
      if (disposed) {
        return;
      }

      const normalizedKey = buildTabKey("album", event.payload.data.url);
      setTabs((current) => tabState.applyAlbumDetail(current, normalizedKey, event.payload.data));
    }).then((value) => {
      unlistenAlbumDetail = value;
    });

    listen<AlbumThumbnailEvent>("album://thumbnail_cached", (event) => {
      const payload = event.payload;
      if (disposed || !payload.local_thumbnail_path) {
        return;
      }

      const normalizedKey = buildTabKey("album", payload.album_url);
      setTabs((current) =>
        tabState.applyAlbumThumbnail(current, normalizedKey, payload.image_id, payload.local_thumbnail_path!),
      );
    }).then((value) => {
      unlistenAlbumThumbnail = value;
    });

    return () => {
      disposed = true;
      unlistenProfile?.();
      unlistenAlbumDetail?.();
      unlistenAlbumThumbnail?.();
    };
  }, []);

  useEffect(() => {
    if (!openTarget || !tabsRestored) {
      return;
    }

    if (openTarget.kind === "album") {
      void openAlbumTab(openTarget.url, false, openTarget.title);
    } else {
      void openProfileTab(openTarget.url, false, openTarget.title);
    }
    onTargetHandled?.(openTarget.id);
  }, [openTarget, tabsRestored]);

  // handleParse 解析输入的 ImgBB 地址。
  async function handleParse() {
    const inputUrl = url.trim();
    if (!inputUrl) {
      message.warning("请输入 ImgBB 地址");
      return;
    }

    setParserLoading(true);
    try {
      const detectedKind = detectParseKind(inputUrl);
      if (!detectedKind) {
        message.warning("未识别为 ImgBB 相册或个人空间地址");
        return;
      }

      if (detectedKind === "album") {
        await openAlbumTab(inputUrl, refresh);
        return;
      }

      await openProfileTab(inputUrl, refresh);
    } finally {
      setParserLoading(false);
    }
  }

  // refreshWebSearchPing 刷新网络搜索检测状态。
  async function refreshWebSearchPing() {
    setAppState({ webSearchPingLoading: true, loadedWebSearchDetect: true });
    try {
      const ping = await pingWebsearch();
      setAppState({ webSearchPing: ping });
    } catch (error) {
      setAppState({
        webSearchPing: {
          engine: "aggregate",
          available: false,
          base_url: "",
          latency_ms: 0,
          error: String(error),
          children: [],
        },
      });
    } finally {
      setAppState({ webSearchPingLoading: false });
    }
  }

  // handleWebSearch 根据关键词搜索公开相册。
  async function handleWebSearch() {
    const keyword = webSearchText.trim();
    if (!keyword) {
      message.warning("请输入搜索关键词");
      return;
    }

    setWebSearchLoading(true);
    try {
      const response = await searchImgbbAlbums(keyword);
      openSearchResultTab(response.query, response.search_query, response.albums);
      if (response.albums.length === 0) {
        const suffix =
          response.errors.length > 0
            ? `；搜索源异常：${response.errors.slice(0, 2).join("；")}`
            : "";
        message.warning(`没有提取到 ImgBB 相册地址，搜索命中 ${response.result_count} 条${suffix}`);
      } else {
        message.success(`已找到 ${response.albums.length} 个相册`);
      }
    } catch (error) {
      message.error(String(error));
    } finally {
      setWebSearchLoading(false);
    }
  }

  // handleImportClipboard 从剪切板导入待解析地址。
  async function handleImportClipboard() {
    if (!navigator.clipboard?.readText) {
      message.error("当前环境不支持读取剪切板");
      return;
    }

    try {
      const text = (await navigator.clipboard.readText()).trim();
      if (!text) {
        message.warning("剪切板没有可导入的内容");
        return;
      }

      setUrl(text);
      message.success("已从剪切板导入");
    } catch (error) {
      message.error(`读取剪切板失败：${String(error)}`);
    }
  }

  // openAlbumTab 打开或复用相册标签并加载详情。
  async function openAlbumTab(inputUrl: string, forceRefresh: boolean, title?: string) {
    const key = buildTabKey("album", inputUrl);
    upsertAlbumTab(key, inputUrl, title);
    activateTab(key);
    void persistParseTab(key, "album", title || "相册", normalizeComparableUrl(inputUrl), true);

    try {
      const response = await parseAlbum(inputUrl, forceRefresh);
      updateAlbumTab(key, response.data, false);
      void persistParseTab(key, "album", response.data.title, response.data.url, true);
      message.success(response.cached ? "已读取相册缓存" : "相册解析完成");
    } catch (error) {
      markTabFailed(key);
      message.error(String(error));
    }
  }

  // openProfileTab 打开或复用个人空间标签并加载相册列表。
  async function openProfileTab(inputUrl: string, forceRefresh: boolean, title?: string) {
    const key = buildTabKey("profile", inputUrl);
    upsertProfileTab(key, inputUrl, title);
    pendingProfileTabKey.current = key;
    activateTab(key);
    void persistParseTab(key, "profile", title || "个人空间", normalizeComparableUrl(inputUrl), true);

    try {
      const response = await parseProfile(inputUrl, forceRefresh);
      updateProfileTab(key, response.data.url, response.data.albums, title, false);
      void persistParseTab(key, "profile", title || profileTitle(response.data.url), response.data.url, true);
      message.success(response.cached ? "已读取个人空间缓存" : "个人空间解析完成");
    } catch (error) {
      markTabFailed(key);
      message.error(String(error));
    } finally {
      if (pendingProfileTabKey.current === key) {
        pendingProfileTabKey.current = undefined;
      }
    }
  }

  // upsertAlbumTab 插入或重置相册标签。
  function upsertAlbumTab(key: string, inputUrl: string, title?: string) {
    setTabs((current) => tabState.upsertAlbumTab(current, key, inputUrl, title));
  }

  // upsertProfileTab 插入或重置个人空间标签。
  function upsertProfileTab(key: string, inputUrl: string, title?: string) {
    setTabs((current) => tabState.upsertProfileTab(current, key, inputUrl, title));
  }

  // openSearchResultTab 打开搜索结果标签。
  function openSearchResultTab(query: string, searchQuery: string, albums: ProfileAlbum[]) {
    const key = buildSearchTabKey(query);
    const title = `搜索：${query}`;
    const tabUrl = `websearch://imgbb-albums?query=${encodeURIComponent(query)}`;

    setTabs((current) => tabState.upsertSearchTab(current, key, title, tabUrl, searchQuery, albums));
    activateTab(key);
  }

  // markTabFailed 标记标签加载失败。
  function markTabFailed(key: string) {
    setTabs((current) => tabState.markTabFailed(current, key));
  }

  // updateAlbumTab 更新相册标签数据。
  function updateAlbumTab(key: string, album: AlbumDetail, loading: boolean) {
    setTabs((current) => tabState.updateAlbumData(current, key, album, loading));
  }

  // updateProfileTab 更新个人空间标签数据。
  function updateProfileTab(
    key: string,
    profileUrl: string,
    albums: ProfileAlbum[],
    title: string | undefined,
    loading: boolean,
  ) {
    setTabs((current) => tabState.updateProfileData(current, key, profileUrl, albums, title, loading));
  }

  // handleCloseTab 关闭结果标签。
  function handleCloseTab(targetKey: string) {
    if (targetKey === PARSER_TAB_KEY) {
      return;
    }

    setTabs((current) => {
      const nextTabs = current.filter((tab) => tab.key !== targetKey);
      const nextKeys = new Set(nextTabs.map((tab) => tab.key));
      tabHistoryRef.current = tabHistoryRef.current.filter(
        (key) => key !== targetKey && nextKeys.has(key),
      );

      if (activeKey === targetKey) {
        const nextActiveKey = tabHistoryRef.current[0] || PARSER_TAB_KEY;
        setActiveKey(nextActiveKey);
        void setActiveParseTab(nextActiveKey === PARSER_TAB_KEY ? undefined : nextActiveKey);
      }

      return nextTabs;
    });
    void removeParseTab(targetKey);
  }

  // rememberTab 维护去重后的最近访问标签栈。
  function rememberTab(tabKey: string) {
    tabHistoryRef.current = [
      tabKey,
      ...tabHistoryRef.current.filter((historyKey) => historyKey !== tabKey),
    ];
  }

  // activateTab 激活标签页并同步本地持久化状态。
  function activateTab(tabKey: string) {
    rememberTab(tabKey);
    setActiveKey(tabKey);
    void setActiveParseTab(tabKey === PARSER_TAB_KEY ? undefined : tabKey);
  }

  // updateAlbumSelection 更新相册图片选择。
  function updateAlbumSelection(tabKey: string, selectedImageIds: string[]) {
    setTabs((current) => tabState.updateAlbumSelection(current, tabKey, selectedImageIds));
  }

  // updateAlbumSearch 更新相册内搜索。
  function updateAlbumSearch(tabKey: string, searchText: string) {
    setTabs((current) => tabState.updateAlbumSearch(current, tabKey, searchText));
  }

  // updateAlbumPage 更新相册当前页。
  function updateAlbumPage(tabKey: string, currentPage: number) {
    setTabs((current) => tabState.updateAlbumPage(current, tabKey, currentPage));
  }

  // updateProfileSelection 更新个人空间相册选择。
  function updateProfileSelection(tabKey: string, selectedAlbumUrls: string[]) {
    setTabs((current) => tabState.updateProfileSelection(current, tabKey, selectedAlbumUrls));
  }

  // updateProfileSearch 更新个人空间内搜索。
  function updateProfileSearch(tabKey: string, searchText: string) {
    setTabs((current) => tabState.updateProfileSearch(current, tabKey, searchText));
  }

  // updateProfilePage 更新个人空间当前页。
  function updateProfilePage(tabKey: string, currentPage: number) {
    setTabs((current) => tabState.updateProfilePage(current, tabKey, currentPage));
  }

  // persistParseTab 保存解析结果标签。
  async function persistParseTab(
    tabKey: string,
    tabKind: ParseKind,
    tabTitle: string,
    tabUrl: string,
    active: boolean,
  ) {
    const existingIndex = tabs.findIndex((tab) => tab.key === tabKey);
    await saveParseTab({
      tab_key: tabKey,
      kind: tabKind,
      title: tabTitle,
      url: tabUrl,
      sort_index: existingIndex >= 0 ? existingIndex : tabs.length,
      active,
    });
  }

  return (
    <ParseTabsView
      tabs={tabs}
      activeKey={activeKey}
      url={url}
      webSearchText={webSearchText}
      refresh={refresh}
      parserLoading={parserLoading}
      webSearchLoading={webSearchLoading}
      webSearchPingLoading={webSearchPingLoading}
      webSearchPing={webSearchPing}
      displaySettings={displaySettings}
      message={message}
      onOpenDownloads={onOpenDownloads}
      onUrlChange={setUrl}
      onWebSearchTextChange={(value) => setAppState({ webSearchText: value })}
      onRefreshChange={setRefresh}
      onParse={handleParse}
      onWebSearch={handleWebSearch}
      onRefreshWebSearchPing={refreshWebSearchPing}
      onImportClipboard={handleImportClipboard}
      onActivateTab={activateTab}
      onCloseTab={handleCloseTab}
      onUpdateAlbumSelection={updateAlbumSelection}
      onUpdateAlbumSearch={updateAlbumSearch}
      onUpdateAlbumPage={updateAlbumPage}
      onUpdateProfileSelection={updateProfileSelection}
      onUpdateProfileSearch={updateProfileSearch}
      onUpdateProfilePage={updateProfilePage}
      onOpenAlbum={openAlbumTab}
      onOpenProfile={openProfileTab}
    />
  );
}
