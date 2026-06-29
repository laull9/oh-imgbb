import { DownloadOutlined, FolderOpenOutlined, HeartOutlined, SearchOutlined } from "@ant-design/icons";
import { Button, Checkbox, Empty, Image, Input, List, Pagination, Space, Spin, Typography } from "antd";
import type { AppSettings, ProfileAlbum } from "../../api/types";
import styles from "../../css/parse_page.module.css";
import type { ProfileTab } from "./types";
import {
  clampPage,
  filterProfileAlbums,
  mergeSelection,
  paginateList,
  profileTitle,
  removeSelection,
} from "./utils";

// ProfileTabViewProps 描述个人空间标签的交互参数。
export interface ProfileTabViewProps {
  tab: ProfileTab;
  displaySettings: Pick<AppSettings, "pagination_enabled" | "profile_page_size">;
  onUpdateSelection: (tabKey: string, selectedAlbumUrls: string[]) => void;
  onUpdateSearch: (tabKey: string, searchText: string) => void;
  onUpdatePage: (tabKey: string, currentPage: number) => void;
  onOpenAlbum: (url: string, refresh: boolean, title?: string) => void;
  onFavoriteProfile: (tab: ProfileTab) => void;
  onFavoriteProfileAlbum: (item: ProfileAlbum) => void;
  onFavoriteSelectedAlbums: (tab: ProfileTab) => void;
  onDownloadSelectedAlbums: (tab: ProfileTab) => void;
}

// ProfileTabView 渲染个人空间或搜索结果相册列表。
export function ProfileTabView({
  tab,
  displaySettings,
  onUpdateSelection,
  onUpdateSearch,
  onUpdatePage,
  onOpenAlbum,
  onFavoriteProfile,
  onFavoriteProfileAlbum,
  onFavoriteSelectedAlbums,
  onDownloadSelectedAlbums,
}: ProfileTabViewProps) {
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
            onChange={(event) => onUpdateSearch(tab.key, event.target.value)}
            placeholder="搜索标题"
            prefix={<SearchOutlined />}
            className={styles.resultSearch}
          />
          <Checkbox
            checked={allProfileSelected}
            onChange={(event) =>
              onUpdateSelection(
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
            <Button icon={<HeartOutlined />} onClick={() => onFavoriteProfile(tab)}>
              收藏空间
            </Button>
          )}
          <Button
            icon={<HeartOutlined />}
            disabled={tab.selectedAlbumUrls.length === 0}
            onClick={() => onFavoriteSelectedAlbums(tab)}
          >
            收藏选中
          </Button>
          <Button
            type="primary"
            icon={<DownloadOutlined />}
            disabled={tab.selectedAlbumUrls.length === 0}
            onClick={() => onDownloadSelectedAlbums(tab)}
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
                  onClick={() => onOpenAlbum(item.url, false, item.name)}
                >
                  打开
                </Button>,
                <Button
                  key="favorite"
                  type="text"
                  icon={<HeartOutlined />}
                  onClick={() => onFavoriteProfileAlbum(item)}
                >
                  收藏
                </Button>,
                <Checkbox
                  key="select"
                  checked={checked}
                  onChange={(event) => {
                    onUpdateSelection(
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
          onChange={(page) => onUpdatePage(tab.key, page)}
        />
      )}
    </Space>
  );
}
