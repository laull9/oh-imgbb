import { DownloadOutlined, ExportOutlined, HeartOutlined, SearchOutlined } from "@ant-design/icons";
import { Button, Checkbox, Empty, Input, Pagination, Space, Typography } from "antd";
import type { AppSettings } from "../../api/types";
import { ThumbnailGrid } from "../../components/thumbnail_grid";
import styles from "../../css/parse_page.module.css";
import type { AlbumTab } from "./types";
import { clampPage, filterAlbumImages, mergeSelection, paginateList, removeSelection } from "./utils";

// AlbumTabViewProps 描述相册标签的交互参数。
export interface AlbumTabViewProps {
  tab: AlbumTab;
  displaySettings: Pick<AppSettings, "pagination_enabled" | "album_page_size">;
  onUpdateSelection: (tabKey: string, selectedImageIds: string[]) => void;
  onFavoriteAlbum: (tab: AlbumTab) => void;
  onDownloadSelectedImages: (tab: AlbumTab) => void;
  onDownloadAlbum: (tab: AlbumTab) => void;
  onOpenProfile: (url: string, refresh: boolean, title?: string) => void;
  onUpdateSearch: (tabKey: string, searchText: string) => void;
  onUpdatePage: (tabKey: string, currentPage: number) => void;
}

// AlbumTabView 渲染相册详情和图片选择操作。
export function AlbumTabView({
  tab,
  displaySettings,
  onUpdateSelection,
  onFavoriteAlbum,
  onDownloadSelectedImages,
  onDownloadAlbum,
  onOpenProfile,
  onUpdateSearch,
  onUpdatePage,
}: AlbumTabViewProps) {
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
                onChange={(event) => onUpdateSearch(tab.key, event.target.value)}
                placeholder="搜索标题"
                prefix={<SearchOutlined />}
                className={styles.resultSearch}
              />
              <Checkbox
                checked={allImageSelected}
                onChange={(event) =>
                  onUpdateSelection(
                    tab.key,
                    event.target.checked
                      ? mergeSelection(tab.selectedImageIds, filteredImageIds)
                      : removeSelection(tab.selectedImageIds, filteredImageIds),
                  )
                }
              >
                全选
              </Checkbox>
              <Button icon={<HeartOutlined />} onClick={() => onFavoriteAlbum(tab)}>
                收藏
              </Button>
              {album.author_url && (
                <Button icon={<ExportOutlined />} onClick={() => onOpenProfile(album.author_url!, false)}>
                  作者空间
                </Button>
              )}
              <Button
                icon={<DownloadOutlined />}
                disabled={tab.selectedImageIds.length === 0}
                onClick={() => onDownloadSelectedImages(tab)}
              >
                下载选中
              </Button>
              <Button type="primary" icon={<DownloadOutlined />} onClick={() => onDownloadAlbum(tab)}>
                下载全部
              </Button>
            </Space>
          </div>
          <ThumbnailGrid
            images={visibleImages}
            selectedIds={tab.selectedImageIds}
            onSelectedIdsChange={(ids) => onUpdateSelection(tab.key, ids)}
          />
          {displaySettings.pagination_enabled && filteredImages.length > displaySettings.album_page_size && (
            <Pagination
              align="end"
              current={albumPage}
              pageSize={displaySettings.album_page_size}
              total={filteredImages.length}
              showSizeChanger={false}
              onChange={(page) => onUpdatePage(tab.key, page)}
            />
          )}
        </>
      ) : (
        <Empty description={tab.loading ? "正在解析相册" : "相册未加载"} />
      )}
    </Space>
  );
}
