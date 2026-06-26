import {
  DownloadOutlined,
  ExportOutlined,
  FolderOpenOutlined,
  HeartOutlined,
  ReloadOutlined,
  SearchOutlined,
  SnippetsOutlined,
} from "@ant-design/icons";
import {
  App,
  Button,
  Card,
  Checkbox,
  Empty,
  Image,
  Input,
  List,
  Pagination,
  Space,
  Spin,
  Switch,
  Tabs,
  Tag,
  Tooltip,
  Typography,
} from "antd";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  downloadAlbum,
  downloadAlbumImages,
  downloadProfileAlbums,
  getSettings,
  listParseTabs,
  parseAlbum,
  parseProfile,
  pingWebsearch,
  removeParseTab,
  saveFavorite,
  saveParseTab,
  searchImgbbAlbums,
  setActiveParseTab,
} from "../api/tauri_client";
import type {
  AlbumDetail,
  AlbumImage,
  AlbumThumbnailEvent,
  AppSettings,
  CachedResponse,
  ParseTabRecord,
  ProfileAlbum,
  ProfileBatch,
  SearchPing,
} from "../api/types";
import { ThumbnailGrid } from "../components/thumbnail_grid";
import styles from "../css/parse_page.module.css";

type ParseKind = "album" | "profile";

export interface ParseOpenTarget {
  id: number;
  kind: ParseKind;
  url: string;
  title?: string;
}

interface ParsePageProps {
  openTarget?: ParseOpenTarget;
  onTargetHandled?: (id: number) => void;
  onOpenDownloads?: () => void;
}

interface ParserTab {
  key: "parser";
  kind: "parser";
  title: string;
}

interface AlbumTab {
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

interface ProfileTab {
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

type ParseTab = ParserTab | AlbumTab | ProfileTab;

const PARSER_TAB_KEY = "parser";
const DEFAULT_DISPLAY_SETTINGS = {
  pagination_enabled: true,
  profile_page_size: 10,
  album_page_size: 20,
};

export function ParsePage({ openTarget, onTargetHandled, onOpenDownloads }: ParsePageProps) {
  const { message } = App.useApp();
  const [url, setUrl] = useState("");
  const [webSearchText, setWebSearchText] = useState("");
  const [refresh, setRefresh] = useState(false);
  const [parserLoading, setParserLoading] = useState(false);
  const [webSearchLoading, setWebSearchLoading] = useState(false);
  const [webSearchPingLoading, setWebSearchPingLoading] = useState(false);
  const [webSearchPing, setWebSearchPing] = useState<SearchPing | undefined>(undefined);
  const [activeKey, setActiveKey] = useState(PARSER_TAB_KEY);
  const [tabs, setTabs] = useState<ParseTab[]>([
    { key: PARSER_TAB_KEY, kind: "parser", title: "解析" },
  ]);
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
    void refreshWebSearchPing();
  }, []);

  useEffect(() => {
    let disposed = false;

    async function restoreTabs() {
      try {
        const records = await listParseTabs();
        if (disposed || records.length === 0) {
          return;
        }

        const restoredActiveKey = records.find((record) => record.active)?.tab_key || PARSER_TAB_KEY;

        setTabs([
          { key: PARSER_TAB_KEY, kind: "parser", title: "解析" },
          ...records.map(recordToTab),
        ]);
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
          if (disposed) {
            return;
          }
          updateAlbumTab(record.tab_key, response.data, false);
          return;
        }

        const response = await parseProfile(record.url, false);
        if (disposed) {
          return;
        }
        updateProfileTab(record.tab_key, response.data.url, response.data.albums, record.title, false);
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

      setTabs((current) =>
        current.map((tab) => {
          if (tab.kind !== "profile" || tab.key !== tabKey) {
            return tab;
          }

          const known = new Set(tab.albums.map((item) => item.url));
          const next = event.payload.albums.filter((item) => !known.has(item.url));
          return { ...tab, albums: [...tab.albums, ...next] };
        }),
      );
    }).then((value) => {
      unlistenProfile = value;
    });

    listen<CachedResponse<AlbumDetail>>("album://detail_ready", (event) => {
      if (disposed) {
        return;
      }

      const normalizedKey = buildTabKey("album", event.payload.data.url);
      setTabs((current) =>
        current.map((tab) => {
          if (tab.kind !== "album" || tab.key !== normalizedKey) {
            return tab;
          }

          return {
            ...tab,
            title: event.payload.data.title,
            url: event.payload.data.url,
            album: event.payload.data,
            selectedImageIds: event.payload.data.images.map((image) => image.id),
            loading: false,
          };
        }),
      );
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
        current.map((tab) => {
          if (tab.kind !== "album" || tab.key !== normalizedKey || !tab.album) {
            return tab;
          }

          return {
            ...tab,
            album: {
              ...tab.album,
              images: tab.album.images.map((image) =>
                image.id === payload.image_id
                  ? { ...image, local_thumbnail_path: payload.local_thumbnail_path }
                  : image,
              ),
            },
          };
        }),
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

  async function refreshWebSearchPing() {
    setWebSearchPingLoading(true);
    try {
      const ping = await pingWebsearch();
      setWebSearchPing(ping);
    } catch (error) {
      setWebSearchPing({
        engine: "aggregate",
        available: false,
        base_url: "",
        latency_ms: 0,
        error: String(error),
        children: [],
      });
    } finally {
      setWebSearchPingLoading(false);
    }
  }

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
        message.warning("没有提取到 ImgBB 相册地址");
      } else {
        message.success(`已找到 ${response.albums.length} 个相册`);
      }
    } catch (error) {
      message.error(String(error));
    } finally {
      setWebSearchLoading(false);
    }
  }

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

  async function openProfileTab(inputUrl: string, forceRefresh: boolean, title?: string) {
    const key = buildTabKey("profile", inputUrl);
    upsertProfileTab(key, inputUrl, title);
    pendingProfileTabKey.current = key;
    activateTab(key);
    void persistParseTab(key, "profile", title || "个人空间", normalizeComparableUrl(inputUrl), true);

    try {
      const response = await parseProfile(inputUrl, forceRefresh);
      updateProfileTab(key, response.data.url, response.data.albums, title, false);
      void persistParseTab(
        key,
        "profile",
        title || profileTitle(response.data.url),
        response.data.url,
        true,
      );
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

  function upsertAlbumTab(key: string, inputUrl: string, title?: string) {
    setTabs((current) => {
      if (current.some((tab) => tab.key === key)) {
        return current.map((tab) =>
          tab.key === key && tab.kind === "album" ? { ...tab, loading: true } : tab,
        );
      }

      return [
        ...current,
        {
          key,
          kind: "album",
          title: title || "相册",
          url: normalizeComparableUrl(inputUrl),
          selectedImageIds: [],
          searchText: "",
          currentPage: 1,
          loading: true,
        },
      ];
    });
  }

  function upsertProfileTab(key: string, inputUrl: string, title?: string) {
    setTabs((current) => {
      if (current.some((tab) => tab.key === key)) {
        return current.map((tab) =>
          tab.key === key && tab.kind === "profile"
            ? { ...tab, albums: [], selectedAlbumUrls: [], loading: true }
            : tab,
        );
      }

      return [
        ...current,
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
        },
      ];
    });
  }

  function openSearchResultTab(query: string, searchQuery: string, albums: ProfileAlbum[]) {
    const key = buildSearchTabKey(query);
    const title = `搜索：${query}`;
    const tabUrl = `websearch://imgbb-albums?query=${encodeURIComponent(query)}`;

    setTabs((current) => {
      if (current.some((tab) => tab.key === key)) {
        return current.map((tab) =>
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
        ...current,
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
        },
      ];
    });
    activateTab(key);
  }

  function markTabFailed(key: string) {
    setTabs((current) =>
      current.map((tab) =>
        tab.key === key && tab.kind !== "parser" ? { ...tab, loading: false } : tab,
      ),
    );
  }

  function updateAlbumTab(key: string, album: AlbumDetail, loading: boolean) {
    setTabs((current) =>
      current.map((tab) =>
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
      ),
    );
  }

  function updateProfileTab(
    key: string,
    profileUrl: string,
    albums: ProfileAlbum[],
    title: string | undefined,
    loading: boolean,
  ) {
    setTabs((current) =>
      current.map((tab) =>
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
      ),
    );
  }

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

  function updateAlbumSelection(tabKey: string, selectedImageIds: string[]) {
    setTabs((current) =>
      current.map((tab) =>
        tab.kind === "album" && tab.key === tabKey ? { ...tab, selectedImageIds } : tab,
      ),
    );
  }

  function updateAlbumSearch(tabKey: string, searchText: string) {
    setTabs((current) =>
      current.map((tab) =>
        tab.kind === "album" && tab.key === tabKey
          ? { ...tab, searchText, currentPage: 1 }
          : tab,
      ),
    );
  }

  function updateAlbumPage(tabKey: string, currentPage: number) {
    setTabs((current) =>
      current.map((tab) =>
        tab.kind === "album" && tab.key === tabKey ? { ...tab, currentPage } : tab,
      ),
    );
  }

  function updateProfileSelection(tabKey: string, selectedAlbumUrls: string[]) {
    setTabs((current) =>
      current.map((tab) =>
        tab.kind === "profile" && tab.key === tabKey ? { ...tab, selectedAlbumUrls } : tab,
      ),
    );
  }

  function updateProfileSearch(tabKey: string, searchText: string) {
    setTabs((current) =>
      current.map((tab) =>
        tab.kind === "profile" && tab.key === tabKey
          ? { ...tab, searchText, currentPage: 1 }
          : tab,
      ),
    );
  }

  function updateProfilePage(tabKey: string, currentPage: number) {
    setTabs((current) =>
      current.map((tab) =>
        tab.kind === "profile" && tab.key === tabKey ? { ...tab, currentPage } : tab,
      ),
    );
  }

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

  async function handleFavoriteAlbum(tab: AlbumTab) {
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

  async function handleFavoriteProfile(tab: ProfileTab) {
    await saveFavorite({
      kind: "profile",
      title: profileTitle(tab.url),
      url: tab.url,
      cover_url: tab.albums[0]?.cover_url,
    });
    message.success("已收藏个人空间");
  }

  async function handleFavoriteSelectedProfileAlbums(tab: ProfileTab) {
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

  async function handleFavoriteProfileAlbum(item: ProfileAlbum) {
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

  async function handleDownloadAlbum(tab: AlbumTab) {
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

  async function handleDownloadSelectedImages(tab: AlbumTab) {
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

  async function handleDownloadSelectedProfileAlbums(tab: ProfileTab) {
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

  const tabItems = useMemo(
    () =>
      tabs.map((tab) => ({
        key: tab.key,
        label: tabLabel(tab),
        closable: tab.key !== PARSER_TAB_KEY,
        children:
          tab.kind === "parser"
            ? renderParserTab(
                url,
                setUrl,
                webSearchText,
                setWebSearchText,
                refresh,
                setRefresh,
                parserLoading,
                webSearchLoading,
                webSearchPingLoading,
                webSearchPing,
                handleParse,
                handleWebSearch,
                refreshWebSearchPing,
                handleImportClipboard,
              )
            : tab.kind === "album"
              ? renderAlbumTab(
                  tab,
                  updateAlbumSelection,
                  handleFavoriteAlbum,
                  handleDownloadSelectedImages,
                  handleDownloadAlbum,
                  openProfileTab,
                  displaySettings,
                  updateAlbumSearch,
                  updateAlbumPage,
                )
              : renderProfileTab(
                  tab,
                  updateProfileSelection,
                  updateProfileSearch,
                  updateProfilePage,
                  openAlbumTab,
                  handleFavoriteProfile,
                  handleFavoriteProfileAlbum,
                  handleFavoriteSelectedProfileAlbums,
                  handleDownloadSelectedProfileAlbums,
                  displaySettings,
                ),
      })),
    [
      tabs,
      url,
      webSearchText,
      refresh,
      parserLoading,
      webSearchLoading,
      webSearchPingLoading,
      webSearchPing,
      displaySettings,
    ],
  );

  return (
    <Tabs
      type="editable-card"
      hideAdd
      activeKey={activeKey}
      onChange={(key) => {
        activateTab(key);
      }}
      onEdit={(targetKey, action) => {
        if (action === "remove") {
          handleCloseTab(String(targetKey));
        }
      }}
      items={tabItems}
      className={styles.browserTabs}
    />
  );
}

function recordToTab(record: ParseTabRecord): ParseTab {
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

function renderParserTab(
  url: string,
  setUrl: (value: string) => void,
  webSearchText: string,
  setWebSearchText: (value: string) => void,
  refresh: boolean,
  setRefresh: (value: boolean) => void,
  parserLoading: boolean,
  webSearchLoading: boolean,
  webSearchPingLoading: boolean,
  webSearchPing: SearchPing | undefined,
  handleParse: () => void,
  handleWebSearch: () => void,
  refreshWebSearchPing: () => void,
  importClipboard: () => void,
) {
  const availableChildren = webSearchPing?.children.filter((item) => item.available) ?? [];
  const pingStatusColor = webSearchPing?.available ? "success" : "error";
  const pingStatusText = webSearchPing
    ? webSearchPing.available
      ? `可用 · ${availableChildren.length || 1} 个入口`
      : "不可用"
    : "未检测";

  return (
    <Space direction="vertical" size={22} className={styles.pageStack}>
      <section className={styles.parserSection}>
        <div className={styles.sectionHeader}>
          <Typography.Title level={3}>地址解析</Typography.Title>
          <div className={styles.sectionDivider} />
        </div>
        <div className={styles.toolbar}>
          <Input
            value={url}
            onChange={(event) => setUrl(event.target.value)}
            onPressEnter={handleParse}
            placeholder="粘贴 ImgBB 相册或个人空间地址"
            prefix={<SearchOutlined />}
            className={styles.urlInput}
          />
          <Button icon={<SnippetsOutlined />} onClick={importClipboard}>
            从剪切板导入
          </Button>
          <Space>
            <Typography.Text>刷新</Typography.Text>
            <Switch checked={refresh} onChange={setRefresh} />
          </Space>
          <Button type="primary" icon={<ReloadOutlined />} loading={parserLoading} onClick={handleParse}>
            解析
          </Button>
        </div>
      </section>
      <section className={styles.parserSection}>
        <div className={styles.sectionHeader}>
          <Typography.Title level={3}>网络搜索</Typography.Title>
          <div className={styles.sectionDivider} />
        </div>
        <div className={styles.webSearchPanel}>
          <div className={styles.webSearchBar}>
            <Input
              value={webSearchText}
              onChange={(event) => setWebSearchText(event.target.value)}
              onPressEnter={handleWebSearch}
              placeholder="网络搜索公开相册"
              prefix={<SearchOutlined />}
              className={styles.webSearchInput}
            />
            <Button type="primary" icon={<SearchOutlined />} loading={webSearchLoading} onClick={handleWebSearch}>
              搜索
            </Button>
          </div>
          <Card size="small" className={styles.pingCard}>
            <div className={styles.pingCardContent}>
              <div>
                <Typography.Text strong>网络搜索检测</Typography.Text>
                <div>
                  <Tag color={pingStatusColor}>{pingStatusText}</Tag>
                  {webSearchPing?.base_url && (
                    <Typography.Text type="secondary">{webSearchPing.base_url}</Typography.Text>
                  )}
                </div>
                {webSearchPing?.error && (
                  <Typography.Text type="secondary" className={styles.pingError}>
                    {webSearchPing.error}
                  </Typography.Text>
                )}
              </div>
              <Button
                icon={<ReloadOutlined />}
                loading={webSearchPingLoading}
                onClick={refreshWebSearchPing}
              >
                检测
              </Button>
            </div>
          </Card>
          <Typography.Text type="secondary" className={styles.webSearchHint}>
            如果不能进行搜索，可能是你的国家对于搜索引擎有一定限制，可以尝试代理或者加速器。
          </Typography.Text>
        </div>
      </section>
      <Empty description="解析结果会在新标签页中打开" />
    </Space>
  );
}

function renderAlbumTab(
  tab: AlbumTab,
  updateSelection: (tabKey: string, selectedImageIds: string[]) => void,
  favoriteAlbum: (tab: AlbumTab) => void,
  downloadSelectedImages: (tab: AlbumTab) => void,
  downloadAlbum: (tab: AlbumTab) => void,
  openProfile: (url: string, refresh: boolean, title?: string) => void,
  displaySettings: Pick<AppSettings, "pagination_enabled" | "album_page_size">,
  updateSearch: (tabKey: string, searchText: string) => void,
  updatePage: (tabKey: string, currentPage: number) => void,
) {
  const album = tab.album;
  const filteredImages = album ? filterAlbumImages(album.images, tab.searchText) : [];
  const albumPage = clampPage(tab.currentPage, filteredImages.length, displaySettings.album_page_size);
  const visibleImages = displaySettings.pagination_enabled
    ? paginateList(filteredImages, albumPage, displaySettings.album_page_size)
    : filteredImages;
  const filteredImageIds = filteredImages.map((image) => image.id);
  const allImageSelected =
    filteredImageIds.length > 0 && filteredImageIds.every((id) => tab.selectedImageIds.includes(id));

  return (
    <Space direction="vertical" size={12} className={styles.pageStack}>
      {album ? (
        <>
          <div className={styles.resultHeader}>
            <div className={styles.resultTitle}>
              <Typography.Title level={4}>{album.title}</Typography.Title>
              <Typography.Text type="secondary">
                {album.url} · {filteredImages.length}/{album.images.length}
              </Typography.Text>
            </div>
            <Space className={styles.resultActions}>
              <Input
                allowClear
                value={tab.searchText}
                onChange={(event) => updateSearch(tab.key, event.target.value)}
                placeholder="搜索标题"
                prefix={<SearchOutlined />}
                className={styles.resultSearch}
              />
              <Checkbox
                checked={allImageSelected}
                onChange={(event) =>
                  updateSelection(
                    tab.key,
                    event.target.checked
                      ? mergeSelection(tab.selectedImageIds, filteredImageIds)
                      : removeSelection(tab.selectedImageIds, filteredImageIds),
                  )
                }
              >
                全选
              </Checkbox>
              <Button icon={<HeartOutlined />} onClick={() => favoriteAlbum(tab)}>
                收藏
              </Button>
              {album.author_url && (
                <Button icon={<ExportOutlined />} onClick={() => openProfile(album.author_url!, false)}>
                  作者空间
                </Button>
              )}
              <Button
                icon={<DownloadOutlined />}
                disabled={tab.selectedImageIds.length === 0}
                onClick={() => downloadSelectedImages(tab)}
              >
                下载选中
              </Button>
              <Button type="primary" icon={<DownloadOutlined />} onClick={() => downloadAlbum(tab)}>
                下载全部
              </Button>
            </Space>
          </div>
          <ThumbnailGrid
            images={visibleImages}
            selectedIds={tab.selectedImageIds}
            onSelectedIdsChange={(ids) => updateSelection(tab.key, ids)}
          />
          {displaySettings.pagination_enabled && filteredImages.length > displaySettings.album_page_size && (
            <Pagination
              align="end"
              current={albumPage}
              pageSize={displaySettings.album_page_size}
              total={filteredImages.length}
              showSizeChanger={false}
              onChange={(page) => updatePage(tab.key, page)}
            />
          )}
        </>
      ) : (
        <Empty description={tab.loading ? "正在解析相册" : "相册未加载"} />
      )}
    </Space>
  );
}

function renderProfileTab(
  tab: ProfileTab,
  updateSelection: (tabKey: string, selectedAlbumUrls: string[]) => void,
  updateSearch: (tabKey: string, searchText: string) => void,
  updatePage: (tabKey: string, currentPage: number) => void,
  openAlbum: (url: string, refresh: boolean, title?: string) => void,
  favoriteProfile: (tab: ProfileTab) => void,
  favoriteProfileAlbum: (item: ProfileAlbum) => void,
  favoriteSelectedAlbums: (tab: ProfileTab) => void,
  downloadSelectedAlbums: (tab: ProfileTab) => void,
  displaySettings: Pick<AppSettings, "pagination_enabled" | "profile_page_size">,
) {
  const filteredAlbums = filterProfileAlbums(tab.albums, tab.searchText);
  const isSearchResult = tab.source === "search";
  const profilePage = clampPage(
    tab.currentPage,
    filteredAlbums.length,
    displaySettings.profile_page_size,
  );
  const visibleAlbums = displaySettings.pagination_enabled
    ? paginateList(filteredAlbums, profilePage, displaySettings.profile_page_size)
    : filteredAlbums;
  const filteredAlbumUrls = filteredAlbums.map((item) => item.url);
  const allProfileSelected =
    filteredAlbumUrls.length > 0 && filteredAlbumUrls.every((url) => tab.selectedAlbumUrls.includes(url));

  return (
    <Space direction="vertical" size={12} className={styles.pageStack}>
      <div className={styles.resultHeader}>
        <div className={styles.resultTitle}>
          <Typography.Title level={4}>{tab.title || profileTitle(tab.url)}</Typography.Title>
          <Typography.Text type="secondary">
            {tab.loading
              ? "解析中"
              : `${filteredAlbums.length}/${tab.albums.length} · 已选择 ${tab.selectedAlbumUrls.length} 个`}
          </Typography.Text>
          {isSearchResult && tab.searchQuery && (
            <Typography.Text type="secondary" className={styles.searchQueryText}>
              {tab.searchQuery}
            </Typography.Text>
          )}
        </div>
        <Space className={styles.resultActions}>
          <Input
            allowClear
            value={tab.searchText}
            onChange={(event) => updateSearch(tab.key, event.target.value)}
            placeholder="搜索标题"
            prefix={<SearchOutlined />}
            className={styles.resultSearch}
          />
          <Checkbox
            checked={allProfileSelected}
            onChange={(event) =>
              updateSelection(
                tab.key,
                event.target.checked
                  ? mergeSelection(tab.selectedAlbumUrls, filteredAlbumUrls)
                  : removeSelection(tab.selectedAlbumUrls, filteredAlbumUrls),
              )
            }
          >
            全选
          </Checkbox>
          {!isSearchResult && (
            <Button icon={<HeartOutlined />} onClick={() => favoriteProfile(tab)}>
              收藏空间
            </Button>
          )}
          <Button
            icon={<HeartOutlined />}
            disabled={tab.selectedAlbumUrls.length === 0}
            onClick={() => favoriteSelectedAlbums(tab)}
          >
            收藏选中
          </Button>
          <Button
            type="primary"
            icon={<DownloadOutlined />}
            disabled={tab.selectedAlbumUrls.length === 0}
            onClick={() => downloadSelectedAlbums(tab)}
          >
            下载选中
          </Button>
        </Space>
      </div>
      <List
        className={styles.profileList}
        dataSource={visibleAlbums}
        locale={{
          emptyText: (
            <Empty description={tab.loading ? "正在解析个人空间" : isSearchResult ? "暂无搜索结果" : "暂无相册"} />
          ),
        }}
        renderItem={(item) => {
          const checked = tab.selectedAlbumUrls.includes(item.url);

          return (
            <List.Item
              actions={[
                <Button
                  key="open"
                  type="text"
                  icon={<FolderOpenOutlined />}
                  onClick={() => openAlbum(item.url, false, item.name)}
                >
                  打开
                </Button>,
                <Button
                  key="favorite"
                  type="text"
                  icon={<HeartOutlined />}
                  onClick={() => favoriteProfileAlbum(item)}
                >
                  收藏
                </Button>,
                <Checkbox
                  key="select"
                  checked={checked}
                  onChange={(event) => {
                    updateSelection(
                      tab.key,
                      event.target.checked
                        ? [...tab.selectedAlbumUrls, item.url]
                        : tab.selectedAlbumUrls.filter((url) => url !== item.url),
                    );
                  }}
                >
                  选择
                </Checkbox>,
              ]}
            >
              <List.Item.Meta
                avatar={
                  item.cover_url ? (
                    <Image
                      width={72}
                      height={72}
                      src={item.cover_url}
                      preview={false}
                      className={styles.profileCoverImage}
                      placeholder={
                        <div className={styles.profileCoverPlaceholder}>
                          <Spin size="small" />
                        </div>
                      }
                    />
                  ) : undefined
                }
                title={item.name}
                description={item.url}
              />
            </List.Item>
          );
        }}
      />
      {displaySettings.pagination_enabled && filteredAlbums.length > displaySettings.profile_page_size && (
        <Pagination
          align="end"
          current={profilePage}
          pageSize={displaySettings.profile_page_size}
          total={filteredAlbums.length}
          showSizeChanger={false}
          onChange={(page) => updatePage(tab.key, page)}
        />
      )}
    </Space>
  );
}

function tabLabel(tab: ParseTab) {
  if (tab.kind === "parser") {
    return tab.title;
  }
  const label = truncateText(tab.title, 18);

  return (
    <Space size={6}>
      <Tooltip title={tab.title}>
        <span className={styles.tabTitleText}>{label}</span>
      </Tooltip>
      {tab.kind === "profile" && <Tag>{tab.source === "search" ? "搜索" : "空间"}</Tag>}
      {tab.loading && <Tag color="processing">加载</Tag>}
    </Space>
  );
}

function settingsToDisplaySettings(settings: AppSettings) {
  return {
    pagination_enabled: settings.pagination_enabled,
    profile_page_size: Math.max(1, settings.profile_page_size || DEFAULT_DISPLAY_SETTINGS.profile_page_size),
    album_page_size: Math.max(1, settings.album_page_size || DEFAULT_DISPLAY_SETTINGS.album_page_size),
  };
}

function filterAlbumImages(images: AlbumImage[], searchText: string) {
  const keyword = normalizeSearchText(searchText);
  if (!keyword) {
    return images;
  }

  return images.filter((image) => normalizeSearchText(image.filename).includes(keyword));
}

function filterProfileAlbums(albums: ProfileAlbum[], searchText: string) {
  const keyword = normalizeSearchText(searchText);
  if (!keyword) {
    return albums;
  }

  return albums.filter((album) => normalizeSearchText(album.name).includes(keyword));
}

function normalizeSearchText(value: string) {
  return value.trim().toLocaleLowerCase();
}

function paginateList<T>(items: T[], currentPage: number, pageSize: number) {
  const safePageSize = Math.max(1, pageSize);
  const safePage = clampPage(currentPage, items.length, safePageSize);
  const start = (safePage - 1) * safePageSize;

  return items.slice(start, start + safePageSize);
}

function clampPage(currentPage: number, total: number, pageSize: number) {
  const safePageSize = Math.max(1, pageSize);
  const maxPage = Math.max(1, Math.ceil(total / safePageSize));

  return Math.min(Math.max(1, currentPage), maxPage);
}

function mergeSelection(current: string[], next: string[]) {
  return Array.from(new Set([...current, ...next]));
}

function removeSelection(current: string[], removed: string[]) {
  const removedSet = new Set(removed);

  return current.filter((item) => !removedSet.has(item));
}

function buildTabKey(kind: ParseKind, inputUrl: string) {
  return `${kind}:${normalizeComparableUrl(inputUrl)}`;
}

function buildSearchTabKey(query: string) {
  return `search:${normalizeSearchText(query)}`;
}

// detectParseKind 根据 URL 形态自动判断解析目标类型。
function detectParseKind(inputUrl: string): ParseKind | undefined {
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

function normalizeComparableUrl(inputUrl: string) {
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

function profileTitle(profileUrl: string) {
  try {
    return new URL(profileUrl).hostname;
  } catch {
    return "个人空间";
  }
}

function truncateText(value: string, maxLength: number) {
  if (value.length <= maxLength) {
    return value;
  }

  return `${value.slice(0, maxLength)}...`;
}
