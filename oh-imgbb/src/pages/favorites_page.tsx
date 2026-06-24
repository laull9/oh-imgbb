import { DeleteOutlined, ReloadOutlined } from "@ant-design/icons";
import { App, Button, Empty, List, Space, Tag, Typography } from "antd";
import { useEffect, useState } from "react";
import { listFavorites, removeFavorite } from "../api/tauri_client";
import type { FavoriteRecord } from "../api/types";

export function FavoritesPage() {
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
    <Space direction="vertical" size={16} className="page-stack">
      <div className="result-header">
        <Typography.Title level={4}>收藏</Typography.Title>
        <Button icon={<ReloadOutlined />} loading={loading} onClick={loadFavorites}>
          刷新
        </Button>
      </div>
      <List
        loading={loading}
        dataSource={favorites}
        locale={{ emptyText: <Empty description="暂无收藏" /> }}
        renderItem={(item) => (
          <List.Item
            actions={[
              <Button
                danger
                type="text"
                icon={<DeleteOutlined />}
                onClick={() => handleRemove(item.id)}
              />,
            ]}
          >
            <List.Item.Meta
              title={
                <Space>
                  <Typography.Text strong>{item.title}</Typography.Text>
                  <Tag>{item.kind}</Tag>
                </Space>
              }
              description={item.url}
            />
          </List.Item>
        )}
      />
    </Space>
  );
}
