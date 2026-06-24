import {
  DownloadOutlined,
  HeartOutlined,
  ReloadOutlined,
  SearchOutlined,
} from "@ant-design/icons";
import {
  App,
  Button,
  Checkbox,
  Empty,
  Image,
  Input,
  List,
  Segmented,
  Space,
  Switch,
  Typography,
} from "antd";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useState } from "react";
import {
  downloadAlbum,
  downloadAlbumImages,
  downloadProfileAlbums,
  parseAlbum,
  parseProfile,
  saveFavorite,
} from "../api/tauri_client";
import type { AlbumDetail, ProfileAlbum, ProfileBatch } from "../api/types";
import { ThumbnailGrid } from "../components/thumbnail_grid";

type ParseKind = "album" | "profile";

export function ParsePage() {
  const { message } = App.useApp();
  const [kind, setKind] = useState<ParseKind>("album");
  const [url, setUrl] = useState("");
  const [refresh, setRefresh] = useState(false);
  const [loading, setLoading] = useState(false);
  const [album, setAlbum] = useState<AlbumDetail>();
  const [profileAlbums, setProfileAlbums] = useState<ProfileAlbum[]>([]);
  const [selectedImageIds, setSelectedImageIds] = useState<string[]>([]);
  const [selectedAlbumUrls, setSelectedAlbumUrls] = useState<string[]>([]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    listen<ProfileBatch>("profile://album_found", (event) => {
      if (disposed || event.payload.finished) {
        return;
      }

      setProfileAlbums((current) => {
        const known = new Set(current.map((item) => item.url));
        const next = event.payload.albums.filter((item) => !known.has(item.url));
        return [...current, ...next];
      });
    }).then((value) => {
      unlisten = value;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const allImageSelected = useMemo(
    () =>
      album !== undefined &&
      album.images.length > 0 &&
      selectedImageIds.length === album.images.length,
    [album, selectedImageIds.length],
  );
  const allProfileSelected = useMemo(
    () => profileAlbums.length > 0 && selectedAlbumUrls.length === profileAlbums.length,
    [profileAlbums.length, selectedAlbumUrls.length],
  );

  async function handleParse() {
    if (!url.trim()) {
      message.warning("请输入 ImgBB 地址");
      return;
    }

    setLoading(true);
    try {
      if (kind === "album") {
        const response = await parseAlbum(url.trim(), refresh);
        setAlbum(response.data);
        setSelectedImageIds(response.data.images.map((image) => image.id));
        message.success(response.cached ? "已读取相册缓存" : "相册解析完成");
        return;
      }

      setProfileAlbums([]);
      setSelectedAlbumUrls([]);
      const response = await parseProfile(url.trim(), refresh);
      setProfileAlbums(response.data.albums);
      message.success(response.cached ? "已读取个人空间缓存" : "个人空间解析完成");
    } catch (error) {
      message.error(String(error));
    } finally {
      setLoading(false);
    }
  }

  async function handleFavoriteAlbum() {
    if (!album) {
      return;
    }

    await saveFavorite({
      kind: "album",
      title: album.title,
      url: album.url,
      cover_url: album.images[0]?.thumbnail_url || album.images[0]?.image_url,
    });
    message.success("已收藏相册");
  }

  async function handleDownloadAlbum() {
    if (!album) {
      return;
    }

    setLoading(true);
    try {
      const report = await downloadAlbum(album.url);
      message.success(`下载完成：${report.downloaded_files} 个文件`);
    } catch (error) {
      message.error(String(error));
    } finally {
      setLoading(false);
    }
  }

  async function handleDownloadSelectedImages() {
    if (!album || selectedImageIds.length === 0) {
      message.warning("请选择要下载的图片");
      return;
    }

    setLoading(true);
    try {
      const report = await downloadAlbumImages(album, selectedImageIds);
      message.success(`下载完成：${report.downloaded_files} 个文件`);
    } catch (error) {
      message.error(String(error));
    } finally {
      setLoading(false);
    }
  }

  async function handleDownloadSelectedProfileAlbums() {
    if (selectedAlbumUrls.length === 0) {
      message.warning("请选择要下载的相册");
      return;
    }

    setLoading(true);
    try {
      const report = await downloadProfileAlbums(selectedAlbumUrls);
      message.success(`批量下载完成：${report.downloaded_files} 个文件`);
    } catch (error) {
      message.error(String(error));
    } finally {
      setLoading(false);
    }
  }

  return (
    <Space direction="vertical" size={16} className="page-stack">
      <div className="toolbar">
        <Segmented
          value={kind}
          onChange={(value) => setKind(value as ParseKind)}
          options={[
            { label: "相册", value: "album" },
            { label: "个人空间", value: "profile" },
          ]}
        />
        <Input
          value={url}
          onChange={(event) => setUrl(event.target.value)}
          onPressEnter={handleParse}
          placeholder="https://ibb.co/album/..."
          prefix={<SearchOutlined />}
          className="url-input"
        />
        <Space>
          <Typography.Text>刷新</Typography.Text>
          <Switch checked={refresh} onChange={setRefresh} />
        </Space>
        <Button type="primary" icon={<ReloadOutlined />} loading={loading} onClick={handleParse}>
          解析
        </Button>
      </div>

      {kind === "album" && (
        <Space direction="vertical" size={12} className="page-stack">
          {album ? (
            <>
              <div className="result-header">
                <div>
                  <Typography.Title level={4}>{album.title}</Typography.Title>
                  <Typography.Text type="secondary">{album.url}</Typography.Text>
                </div>
                <Space>
                  <Checkbox
                    checked={allImageSelected}
                    onChange={(event) =>
                      setSelectedImageIds(
                        event.target.checked ? album.images.map((image) => image.id) : [],
                      )
                    }
                  >
                    全选
                  </Checkbox>
                  <Button icon={<HeartOutlined />} onClick={handleFavoriteAlbum}>
                    收藏
                  </Button>
                  <Button
                    icon={<DownloadOutlined />}
                    disabled={selectedImageIds.length === 0}
                    loading={loading}
                    onClick={handleDownloadSelectedImages}
                  >
                    下载选中
                  </Button>
                  <Button
                    type="primary"
                    icon={<DownloadOutlined />}
                    loading={loading}
                    onClick={handleDownloadAlbum}
                  >
                    下载全部
                  </Button>
                </Space>
              </div>
              <ThumbnailGrid
                images={album.images}
                selectedIds={selectedImageIds}
                onSelectedIdsChange={setSelectedImageIds}
              />
            </>
          ) : (
            <Empty description="等待解析相册" />
          )}
        </Space>
      )}

      {kind === "profile" && (
        <Space direction="vertical" size={12} className="page-stack">
          <div className="result-header">
            <div>
              <Typography.Title level={4}>个人空间相册</Typography.Title>
              <Typography.Text type="secondary">已选择 {selectedAlbumUrls.length} 个</Typography.Text>
            </div>
            <Space>
              <Checkbox
                checked={allProfileSelected}
                onChange={(event) =>
                  setSelectedAlbumUrls(
                    event.target.checked ? profileAlbums.map((item) => item.url) : [],
                  )
                }
              >
                全选
              </Checkbox>
              <Button
                type="primary"
                icon={<DownloadOutlined />}
                disabled={selectedAlbumUrls.length === 0}
                loading={loading}
                onClick={handleDownloadSelectedProfileAlbums}
              >
                下载选中
              </Button>
            </Space>
          </div>
          <List
            className="profile-list"
            dataSource={profileAlbums}
            locale={{ emptyText: <Empty description="等待解析个人空间" /> }}
            renderItem={(item) => {
              const checked = selectedAlbumUrls.includes(item.url);

              return (
                <List.Item
                  actions={[
                    <Checkbox
                      key="select"
                      checked={checked}
                      onChange={(event) => {
                        setSelectedAlbumUrls((current) =>
                          event.target.checked
                            ? [...current, item.url]
                            : current.filter((url) => url !== item.url),
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
                        <Image width={72} height={72} src={item.cover_url} preview={false} />
                      ) : undefined
                    }
                    title={item.name}
                    description={item.url}
                  />
                </List.Item>
              );
            }}
          />
        </Space>
      )}
    </Space>
  );
}
