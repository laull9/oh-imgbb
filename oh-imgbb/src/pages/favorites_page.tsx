import { DeleteOutlined, FolderOpenOutlined, ReloadOutlined } from "@ant-design/icons";
import { App, Button, Card, Empty, Space, Tag, Typography } from "antd";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { listFavorites, removeFavorite } from "../api/tauri_client";
import type { FavoriteRecord } from "../api/types";
import type { ParseOpenTarget } from "./parse_page";
import styles from "../css/favorites_page.module.css";

interface FavoritesPageProps {
  onOpenTarget?: (target: Omit<ParseOpenTarget, "id">) => void;
}

export function FavoritesPage({ onOpenTarget }: FavoritesPageProps) {
  const { message } = App.useApp();
  const [favorites, setFavorites] = useState<FavoriteRecord[]>([]);
  const [loading, setLoading] = useState(false);

  async function loadFavorites() {
    setLoading(true);
    try {
      setFavorites(await listFavorites());
    } catch (error) {
      message.error(String(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadFavorites();
  }, []);

  async function handleRemove(id: number) {
    await removeFavorite(id);
    await loadFavorites();
    message.success("已删除收藏");
  }

  return (
    <Space direction="vertical" size={16} className={styles.pageStack}>
      <div className={styles.resultHeader}>
        <Typography.Title level={4}>收藏</Typography.Title>
        <Button icon={<ReloadOutlined />} loading={loading} onClick={loadFavorites}>
          刷新
        </Button>
      </div>
      {favorites.length === 0 && !loading ? (
        <div className={styles.empty}>
          <Empty description="暂无收藏" />
        </div>
      ) : (
        <div className={styles.grid} aria-busy={loading}>
          {favorites.map((item) => {
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
                    <Tag>{item.kind === "profile" ? "空间" : "相册"}</Tag>
                  </Space>
                  <Typography.Text type="secondary" ellipsis title={item.url}>
                    {item.url}
                  </Typography.Text>
                </Space>
              </Card>
            );
          })}
        </div>
      )}
    </Space>
  );
}
