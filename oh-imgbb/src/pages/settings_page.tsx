import { DeleteOutlined, SaveOutlined } from "@ant-design/icons";
import { App, Button, Form, Input, InputNumber, Space, Switch, Typography } from "antd";
import { useEffect, useState } from "react";
import { clearThumbnailCache, getSettings, updateSettings } from "../api/tauri_client";
import type { AppSettings } from "../api/types";
import styles from "../css/settings_page.module.css";

export function SettingsPage() {
  const { message } = App.useApp();
  const [form] = Form.useForm<AppSettings>();
  const [loading, setLoading] = useState(false);
  const [clearing, setClearing] = useState(false);

  useEffect(() => {
    getSettings()
      .then((settings) => form.setFieldsValue(settings))
      .catch((error) => message.error(String(error)));
  }, [form, message]);

  async function handleFinish(values: AppSettings) {
    setLoading(true);
    try {
      const saved = await updateSettings(values);
      form.setFieldsValue(saved);
      message.success("设置已保存");
    } catch (error) {
      message.error(String(error));
    } finally {
      setLoading(false);
    }
  }

  async function handleClearThumbnailCache() {
    setClearing(true);
    try {
      await clearThumbnailCache();
      message.success("缩略图缓存已清空");
    } catch (error) {
      message.error(String(error));
    } finally {
      setClearing(false);
    }
  }

  return (
    <div className={styles.panel}>
      <Typography.Title level={4}>下载与缓存</Typography.Title>
      <Form form={form} layout="vertical" onFinish={handleFinish}>
        <Form.Item label="下载目录" name="download_dir" rules={[{ required: true }]}>
          <Input />
        </Form.Item>
        <Form.Item label="文件命名模板" name="file_name_pattern">
          <Input placeholder="{album}_{count}_{name}" />
        </Form.Item>
        <Form.Item label="最大并发下载数" name="max_concurrent_downloads">
          <InputNumber min={1} max={32} />
        </Form.Item>
        <Form.Item label="请求重试次数" name="max_retries">
          <InputNumber min={0} max={10} />
        </Form.Item>
        <Form.Item label="缩略图缓存上限 MB" name="thumbnail_cache_limit_mb">
          <InputNumber min={64} max={8192} />
        </Form.Item>
        <Form.Item label="启用缩略图缓存" name="thumbnail_cache_enabled" valuePropName="checked">
          <Switch />
        </Form.Item>
        <Form.Item label="启动时恢复上次页面" name="restore_last_page" valuePropName="checked">
          <Switch />
        </Form.Item>
        <Typography.Title level={4}>解析显示</Typography.Title>
        <Form.Item label="启用分页" name="pagination_enabled" valuePropName="checked">
          <Switch />
        </Form.Item>
        <Form.Item label="个人空间每页相册数" name="profile_page_size">
          <InputNumber min={1} max={200} />
        </Form.Item>
        <Form.Item label="相册每页图片数" name="album_page_size">
          <InputNumber min={1} max={500} />
        </Form.Item>
        <Space className={styles.actions}>
          <Button type="primary" htmlType="submit" icon={<SaveOutlined />} loading={loading}>
            保存
          </Button>
          <Button icon={<DeleteOutlined />} loading={clearing} onClick={handleClearThumbnailCache}>
            清空缩略图缓存
          </Button>
        </Space>
      </Form>
    </div>
  );
}
