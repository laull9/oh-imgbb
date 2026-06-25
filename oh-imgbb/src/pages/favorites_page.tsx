import { DeleteOutlined, FolderOpenOutlined, ReloadOutlined, SearchOutlined } from "@ant-design/icons";
import { App, Button, Card, Empty, Input, Pagination, Space, Tabs, Tag, Typography } from "antd";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import { listFavorites, removeFavorite } from "../api/tauri_client";
import type { FavoriteRecord } from "../api/types";
import type { ParseOpenTarget } from "./parse_page";
import styles from "../css/favorites_page.module.css";

type FavoriteKind = "profile" | "album";

interface FavoritesPageProps {
  onOpenTarget?: (target: Omit<ParseOpenTarget, "id">) => void;
}

const DEFAULT_PAGE_SIZE = 12;

export function FavoritesPage({ onOpenTarget }: FavoritesPageProps) {
  const { message } = App.useApp();
  const [favoriteKind, setFavoriteKind] = useState<FavoriteKind>("profile");
  const [favorites, setFavorites] = useState<FavoriteRecord[]>([]);
  const [searchText, setSearchText] = useState("");
  const [currentPage, setCurrentPage] = useState(1);
  const [pageSize, setPageSize] = useState(DEFAULT_PAGE_SIZE);
  const [loading, setLoading] = useState(false);

  async function loadFavorites() {
    setLoading(true);
    try {
      setFavorites(await listFavorites(favoriteKind));
    } catch (error) {
      message.error(String(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadFavorites();
  }, [favoriteKind]);

  async function handleRemove(id: number) {
    await removeFavorite(id);
    await loadFavorites();
    message.success("已删除收藏");
  }

  const filteredFavorites = useMemo(
    () => filterFavorites(favorites, searchText),
    [favorites, searchText],
  );
  const favoritePage = clampFavoritePage(currentPage, filteredFavorites.length, pageSize);
  const visibleFavorites = useMemo(
    () => paginateFavorites(filteredFavorites, favoritePage, pageSize),
    [filteredFavorites, favoritePage, pageSize],
  );

  function handleKindChange(key: string) {
    setFavoriteKind(key as FavoriteKind);
    setSearchText("");
    setCurrentPage(1);
  }

  function renderFavorites() {
    if (visibleFavorites.length === 0 && !loading) {
      return (
        <div className={styles.empty}>
          <Empty description={searchText ? "没有匹配的收藏" : `暂无${favoriteKindLabel(favoriteKind)}`} />
        </div>
      );
    }

    return (
      <>
        <div className={styles.grid} aria-busy={loading}>
          {visibleFavorites.map((item) => {
            const coverUrl = item.local_cover_path
              ? convertFileSrc(item.local_cover_path)
              : item.cover_url;

            return (
              <Card
                key={item.id}
                className={styles.card}
                loading={loading}
                actions={[
                  <Button
                    key="open"
                    type="text"
                    icon={<FolderOpenOutlined />}
                    onClick={() =>
                      onOpenTarget?.({
                        kind: item.kind === "profile" ? "profile" : "album",
                        url: item.url,
                        title: item.title,
                      })
                    }
                  >
                    打开
                  </Button>,
                  <Button
                    key="delete"
                    danger
                    type="text"
                    icon={<DeleteOutlined />}
                    onClick={() => handleRemove(item.id)}
                  >
                    删除
                  </Button>,
                ]}
              >
                <div className={styles.cover}>
                  {coverUrl ? <img src={coverUrl} alt={item.title} loading="lazy" /> : <span />}
                </div>
                <Space direction="vertical" size={8} className={styles.body}>
                  <Space size={8} className={styles.titleRow}>
                    <Typography.Text strong ellipsis title={item.title}>
                      {item.title}
                    </Typography.Text>
                    <Tag>{favoriteKindLabel(item.kind as FavoriteKind)}</Tag>
                  </Space>
                  <Typography.Text type="secondary" ellipsis title={item.url}>
                    {item.url}
                  </Typography.Text>
                </Space>
              </Card>
            );
          })}
        </div>
        {filteredFavorites.length > pageSize && (
          <Pagination
            align="end"
            current={favoritePage}
            pageSize={pageSize}
            total={filteredFavorites.length}
            showSizeChanger
            pageSizeOptions={[12, 24, 48]}
            onChange={(page, size) => {
              setCurrentPage(page);
              setPageSize(size);
            }}
          />
        )}
      </>
    );
  }

  return (
    <Space direction="vertical" size={16} className={styles.pageStack}>
      <div className={styles.resultHeader}>
        <Typography.Title level={4}>收藏</Typography.Title>
        <Space className={styles.headerActions}>
          <Input
            allowClear
            value={searchText}
            onChange={(event) => {
              setSearchText(event.target.value);
              setCurrentPage(1);
            }}
            placeholder={`搜索${favoriteKindLabel(favoriteKind)}`}
            prefix={<SearchOutlined />}
            className={styles.searchInput}
          />
          <Button icon={<ReloadOutlined />} loading={loading} onClick={loadFavorites}>
            刷新
          </Button>
        </Space>
      </div>
      <Tabs
        activeKey={favoriteKind}
        onChange={handleKindChange}
        items={[
          {
            key: "profile",
            label: "个人空间",
            children: favoriteKind === "profile" ? renderFavorites() : undefined,
          },
          {
            key: "album",
            label: "相册",
            children: favoriteKind === "album" ? renderFavorites() : undefined,
          },
        ]}
      />
    </Space>
  );
}

// filterFavorites 按标题、地址和备注搜索收藏。
function filterFavorites(favorites: FavoriteRecord[], searchText: string) {
  const keyword = normalizeSearchText(searchText);
  if (!keyword) {
    return favorites;
  }

  return favorites.filter((item) =>
    [item.title, item.url, item.note || ""].some((value) =>
      normalizeSearchText(value).includes(keyword),
    ),
  );
}

// paginateFavorites 返回当前页的收藏项。
function paginateFavorites(favorites: FavoriteRecord[], currentPage: number, pageSize: number) {
  const safePageSize = Math.max(1, pageSize);
  const safePage = clampFavoritePage(currentPage, favorites.length, safePageSize);
  const start = (safePage - 1) * safePageSize;

  return favorites.slice(start, start + safePageSize);
}

// clampFavoritePage 防止搜索或删除后页码越界。
function clampFavoritePage(currentPage: number, total: number, pageSize: number) {
  const safePageSize = Math.max(1, pageSize);
  const maxPage = Math.max(1, Math.ceil(total / safePageSize));

  return Math.min(Math.max(1, currentPage), maxPage);
}

// favoriteKindLabel 返回收藏类型的中文名称。
function favoriteKindLabel(kind: FavoriteKind) {
  return kind === "profile" ? "个人空间" : "相册";
}

// normalizeSearchText 统一搜索文本大小写和空白。
function normalizeSearchText(value: string) {
  return value.trim().toLocaleLowerCase();
}
