import {
  DeleteOutlined,
  DownloadOutlined,
  ExportOutlined,
  FolderOpenOutlined,
  LoginOutlined,
  LogoutOutlined,
  ReloadOutlined,
  UploadOutlined,
} from "@ant-design/icons";
import { Button, Checkbox, Empty, Form, Image, Input, List, Pagination, Space, Spin, Typography } from "antd";
import type { FormInstance } from "antd";
import type {
  AlbumDetail,
  AlbumImage,
  AppSettings,
  IbbAlbumPrivacy,
  IbbCreateAlbumInput,
  LoginStatus,
  ProfileAlbum,
} from "../api/types";
import { ThumbnailGrid } from "../components/thumbnail_grid";
import parseStyles from "../css/parse_page.module.css";
import styles from "../css/mine_page.module.css";

export interface LoginForm {
  login_subject: string;
  password: string;
}

export interface CreateAlbumForm {
  name: string;
  description?: string;
  privacy: IbbAlbumPrivacy;
  password?: string;
}

export interface DisplaySettings {
  pagination_enabled: boolean;
  profile_page_size: number;
  album_page_size: number;
}

export interface ManagedAlbumTab {
  key: string;
  title: string;
  url: string;
  album?: AlbumDetail;
  selectedImageIds: string[];
  searchText: string;
  currentPage: number;
  loading: boolean;
  uploadLoading: boolean;
  deletingImageIds: string[];
}

export interface MineAlbumListProps {
  visibleItems: ProfileAlbum[];
  total: number;
  displaySettings: DisplaySettings;
  profilePage: number;
  profileLoading: boolean;
  deletingAlbumUrls: string[];
  onPageChange: (page: number) => void;
  onOpenAlbum: (item: ProfileAlbum) => void;
  onDeleteAlbum: (item: ProfileAlbum) => void;
  onOpenInBrowser: (url: string) => void;
}

export interface ManagedAlbumRenderProps {
  tab: ManagedAlbumTab;
  displaySettings: DisplaySettings;
  onUpdateTab: (tabKey: string, patch: Partial<ManagedAlbumTab>) => void;
  onRefreshAlbum: (item: ProfileAlbum) => void;
  onChooseUploadFiles: (tabKey: string) => void;
  onDeleteImage: (tabKey: string, image: AlbumImage) => void;
  onDownloadSelectedImages: (tab: ManagedAlbumTab) => void;
  onDownloadAlbum: (tab: ManagedAlbumTab) => void;
  onDeleteSelectedImages: (tab: ManagedAlbumTab) => void;
  onOpenInBrowser: (url: string) => void;
}

export const IMAGE_EXTENSIONS = ["jpg", "jpeg", "png", "gif", "webp", "avif", "bmp", "svg"];

const DEFAULT_DISPLAY_SETTINGS = {
  pagination_enabled: true,
  profile_page_size: 10,
  album_page_size: 20,
};

// renderLoginTab 渲染登录表单和当前状态。
export function renderLoginTab(
  form: FormInstance<LoginForm>,
  status: LoginStatus,
  authLoading: boolean,
  logoutLoading: boolean,
  onLogin: (values: LoginForm) => void,
  onLogout: () => void,
) {
  return (
    <div className={styles.panel}>
      <Typography.Title level={4}>ImgBB 登录</Typography.Title>
      <div className={styles.authStatus}>
        <Typography.Text type={status.verified ? "success" : "secondary"}>
          {status.logged_in ? `已验证：${status.login_subject ?? "ImgBB"}` : "未登录"}
        </Typography.Text>
        {status.profile_url && <Typography.Text type="secondary">{status.profile_url}</Typography.Text>}
      </div>
      <Form form={form} layout="vertical" onFinish={onLogin}>
        <Form.Item label="用户名" name="login_subject" rules={[{ required: true }]}>
          <Input autoComplete="username" />
        </Form.Item>
        <Form.Item label="密码" name="password" rules={[{ required: true }]}>
          <Input.Password autoComplete="current-password" />
        </Form.Item>
        <Space className={styles.actions}>
          <Button type="primary" htmlType="submit" icon={<LoginOutlined />} loading={authLoading}>
            登录
          </Button>
          <Button
            icon={<LogoutOutlined />}
            disabled={!status.logged_in}
            loading={logoutLoading}
            onClick={onLogout}
          >
            退出
          </Button>
        </Space>
      </Form>
    </div>
  );
}

// renderMineAlbumList 渲染当前账号相册列表。
export function renderMineAlbumList({
  visibleItems,
  total,
  displaySettings,
  profilePage,
  profileLoading,
  deletingAlbumUrls,
  onPageChange,
  onOpenAlbum,
  onDeleteAlbum,
  onOpenInBrowser,
}: MineAlbumListProps) {
  return (
    <>
      <List
        className={styles.profileList}
        loading={profileLoading}
        dataSource={visibleItems}
        locale={{ emptyText: <Empty description="暂无相册" /> }}
        renderItem={(item) => (
          <List.Item
            actions={[
              <Button
                key="open"
                type="text"
                icon={<FolderOpenOutlined />}
                onClick={() => onOpenAlbum(item)}
              >
                打开
              </Button>,
              <Button
                key="browser"
                type="text"
                icon={<ExportOutlined />}
                onClick={() => onOpenInBrowser(item.url)}
              >
                浏览器
              </Button>,
              <Button
                key="delete"
                danger
                type="text"
                icon={<DeleteOutlined />}
                loading={deletingAlbumUrls.includes(item.url)}
                onClick={() => onDeleteAlbum(item)}
              >
                删除
              </Button>,
            ]}
          >
            <List.Item.Meta
              avatar={
                item.cover_url ? (
                  <Image width={72} height={72} src={item.cover_url} preview={false} />
                ) : undefined
              }
              title={
                <button className={styles.linkButton} onClick={() => onOpenAlbum(item)}>
                  {item.name}
                </button>
              }
              description={item.url}
            />
          </List.Item>
        )}
      />
      {displaySettings.pagination_enabled && total > displaySettings.profile_page_size && (
        <Pagination
          align="end"
          current={profilePage}
          pageSize={displaySettings.profile_page_size}
          total={total}
          showSizeChanger={false}
          onChange={onPageChange}
        />
      )}
    </>
  );
}

// renderManagedAlbumTab 渲染管理版相册标签。
export function renderManagedAlbumTab({
  tab,
  displaySettings,
  onUpdateTab,
  onRefreshAlbum,
  onChooseUploadFiles,
  onDeleteImage,
  onDownloadSelectedImages,
  onDownloadAlbum,
  onDeleteSelectedImages,
  onOpenInBrowser,
}: ManagedAlbumRenderProps) {
  if (tab.loading) {
    return (
      <div className={styles.centerState}>
        <Spin tip="正在打开相册" />
      </div>
    );
  }
  if (!tab.album) {
    return (
      <div className={styles.centerState}>
        <Empty description="相册未加载" image={Empty.PRESENTED_IMAGE_SIMPLE} />
      </div>
    );
  }
  const filteredImages = filterAlbumImages(tab.album.images, tab.searchText);
  const albumPage = clampPage(tab.currentPage, filteredImages.length, displaySettings.album_page_size);
  const visibleImages = displaySettings.pagination_enabled
    ? paginateList(filteredImages, albumPage, displaySettings.album_page_size)
    : filteredImages;
  const filteredImageIds = filteredImages.map((image) => image.id);
  const allImageSelected =
    filteredImageIds.length > 0 && filteredImageIds.every((id) => tab.selectedImageIds.includes(id));

  return (
    <Space direction="vertical" size={12} className={parseStyles.pageStack}>
      <div className={parseStyles.resultHeader}>
        <div className={parseStyles.resultTitle}>
          <Typography.Title level={4}>{tab.album.title}</Typography.Title>
          <Typography.Text type="secondary">
            {filteredImages.length}/{tab.album.images.length} · 已选 {tab.selectedImageIds.length} · 可拖动图片到窗口上传
          </Typography.Text>
        </div>
        <Space className={parseStyles.resultActions}>
          <Input.Search
            allowClear
            value={tab.searchText}
            onChange={(event) => onUpdateTab(tab.key, { searchText: event.target.value, currentPage: 1 })}
            placeholder="搜索图片"
            className={parseStyles.resultSearch}
          />
          <Checkbox
            checked={allImageSelected}
            onChange={(event) =>
              onUpdateTab(tab.key, {
                selectedImageIds: event.target.checked
                  ? mergeSelection(tab.selectedImageIds, filteredImageIds)
                  : removeSelection(tab.selectedImageIds, filteredImageIds),
              })
            }
          >
            全选
          </Checkbox>
          <Button icon={<ReloadOutlined />} onClick={() => onRefreshAlbum(profileAlbumFromDetail(tab.album!))}>
            刷新
          </Button>
          <Button icon={<ExportOutlined />} onClick={() => onOpenInBrowser(tab.album!.url)}>
            在浏览器中打开
          </Button>
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
          <Button
            danger
            icon={<DeleteOutlined />}
            disabled={tab.selectedImageIds.length === 0}
            onClick={() => onDeleteSelectedImages(tab)}
          >
            删除选中
          </Button>
          <Button
            icon={<UploadOutlined />}
            loading={tab.uploadLoading}
            onClick={() => onChooseUploadFiles(tab.key)}
          >
            上传图片
          </Button>
        </Space>
      </div>
      <div className={styles.dropHint}>
        <ThumbnailGrid
          images={visibleImages}
          selectedIds={tab.selectedImageIds}
          onSelectedIdsChange={(ids) => onUpdateTab(tab.key, { selectedImageIds: ids })}
          detailReferer={tab.album.url}
          onDeleteImage={(image) => onDeleteImage(tab.key, image)}
          deletingImageIds={tab.deletingImageIds}
        />
      </div>
      {displaySettings.pagination_enabled && filteredImages.length > displaySettings.album_page_size && (
        <Pagination
          align="end"
          current={albumPage}
          pageSize={displaySettings.album_page_size}
          total={filteredImages.length}
          showSizeChanger={false}
          onChange={(page) => onUpdateTab(tab.key, { currentPage: page })}
        />
      )}
    </Space>
  );
}

// settingsToDisplaySettings 转换分页设置。
export function settingsToDisplaySettings(settings?: AppSettings): DisplaySettings {
  return {
    pagination_enabled: settings?.pagination_enabled ?? DEFAULT_DISPLAY_SETTINGS.pagination_enabled,
    profile_page_size: Math.max(1, settings?.profile_page_size || DEFAULT_DISPLAY_SETTINGS.profile_page_size),
    album_page_size: Math.max(1, settings?.album_page_size || DEFAULT_DISPLAY_SETTINGS.album_page_size),
  };
}

// valuesToCreateAlbumInput 转换创建相册表单。
export function valuesToCreateAlbumInput(values: CreateAlbumForm): IbbCreateAlbumInput {
  return {
    name: values.name,
    description: values.description,
    privacy: values.privacy,
    password: values.password,
  };
}

// filterProfileAlbums 按名称过滤相册。
export function filterProfileAlbums(albums: ProfileAlbum[], searchText: string) {
  const keyword = normalizeSearchText(searchText);
  return keyword ? albums.filter((item) => normalizeSearchText(item.name).includes(keyword)) : albums;
}

// filterAlbumImages 按文件名过滤图片。
export function filterAlbumImages(images: AlbumImage[], searchText: string) {
  const keyword = normalizeSearchText(searchText);
  return keyword ? images.filter((image) => normalizeSearchText(image.filename).includes(keyword)) : images;
}

// paginateList 截取当前分页数据。
export function paginateList<T>(items: T[], currentPage: number, pageSize: number) {
  const safePageSize = Math.max(1, pageSize);
  const safePage = clampPage(currentPage, items.length, safePageSize);
  const start = (safePage - 1) * safePageSize;
  return items.slice(start, start + safePageSize);
}

// clampPage 将页码限制到合法范围。
export function clampPage(currentPage: number, total: number, pageSize: number) {
  const safePageSize = Math.max(1, pageSize);
  const maxPage = Math.max(1, Math.ceil(total / safePageSize));

  return Math.min(Math.max(1, currentPage), maxPage);
}

// mergeSelection 合并选择项并去重。
export function mergeSelection(current: string[], next: string[]) {
  return Array.from(new Set([...current, ...next]));
}

// removeSelection 从选择项中移除指定项。
export function removeSelection(current: string[], removed: string[]) {
  const removedSet = new Set(removed);

  return current.filter((item) => !removedSet.has(item));
}

// extractAlbumId 从 ImgBB 相册地址提取相册 ID。
export function extractAlbumId(url: string) {
  const parsed = new URL(url);
  const albumId = parsed.pathname.split("/").filter(Boolean)[1];
  if (!albumId) {
    throw new Error(`无法识别相册 ID：${url}`);
  }
  return albumId;
}

// buildAlbumTabKey 构造账号相册管理标签 key。
export function buildAlbumTabKey(url: string) {
  return `album:${extractAlbumId(url)}`;
}

// isAlbumTabKey 判断是否为相册管理标签。
export function isAlbumTabKey(key: string) {
  return key.startsWith("album:");
}

// profileAlbumFromDetail 将相册详情转换为个人空间相册项。
export function profileAlbumFromDetail(album: AlbumDetail): ProfileAlbum {
  return {
    name: album.title,
    url: album.url,
    cover_url: album.images[0]?.thumbnail_url || album.images[0]?.image_url,
  };
}

// isImagePath 判断本地路径是否为支持的图片类型。
export function isImagePath(path: string) {
  const extension = path.split(".").pop()?.toLowerCase();
  return Boolean(extension && IMAGE_EXTENSIONS.includes(extension));
}

// normalizeSearchText 规整搜索关键字。
function normalizeSearchText(value: string) {
  return value.trim().toLocaleLowerCase();
}
