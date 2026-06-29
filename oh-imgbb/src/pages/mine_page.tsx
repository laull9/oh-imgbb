import { FolderAddOutlined, LoginOutlined, ReloadOutlined } from "@ant-design/icons";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { App, Button, Empty, Form, Input, Modal, Radio, Space, Tabs, Typography } from "antd";
import { useEffect, useRef, useState } from "react";
import {
  createImgbbAlbum,
  deleteImgbbAlbum,
  deleteImgbbImage,
  getImgbbLoginStatus,
  getSettings,
  loginImgbb,
  logoutImgbb,
  parseAlbum,
  parseProfile,
  uploadImgbbAlbumImage,
} from "../api/tauri_client";
import type { AlbumImage, AppSettings, ProfileAlbum } from "../api/types";
import parseStyles from "../css/parse_page.module.css";
import { useAppStore } from "../tools/store";
import {
  IMAGE_EXTENSIONS,
  type ManagedAlbumTab,
  type CreateAlbumForm,
  type LoginForm,
  buildAlbumTabKey,
  extractAlbumId,
  filterProfileAlbums,
  isAlbumTabKey,
  isImagePath,
  paginateList,
  renderLoginTab,
  renderManagedAlbumTab,
  renderMineAlbumList,
  settingsToDisplaySettings,
  valuesToCreateAlbumInput,
} from "./mine_page_shared";

const LOGIN_TAB_KEY = "login";
const ACCOUNT_TAB_KEY = "account";

// MinePage 展示登录、个人空间和管理版相册标签。
export function MinePage() {
  const { message, modal } = App.useApp();
  const [loginForm] = Form.useForm<LoginForm>();
  const [createForm] = Form.useForm<CreateAlbumForm>();
  const loginStatus = useAppStore((state) => state.loginStatus);
  const setAppState = useAppStore((state) => state.setState);
  const [activeTab, setActiveTab] = useState(LOGIN_TAB_KEY);
  const [settings, setSettings] = useState<AppSettings>();
  const [authLoading, setAuthLoading] = useState(false);
  const [logoutLoading, setLogoutLoading] = useState(false);
  const [profileLoading, setProfileLoading] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [createLoading, setCreateLoading] = useState(false);
  const [albums, setAlbums] = useState<ProfileAlbum[]>([]);
  const [profileSearch, setProfileSearch] = useState("");
  const [profilePage, setProfilePage] = useState(1);
  const [albumTabs, setAlbumTabs] = useState<ManagedAlbumTab[]>([]);
  const [deletingAlbumUrls, setDeletingAlbumUrls] = useState<string[]>([]);
  const albumTabsRef = useRef<ManagedAlbumTab[]>([]);
  const activeTabRef = useRef(activeTab);

  useEffect(() => {
    activeTabRef.current = activeTab;
  }, [activeTab]);

  useEffect(() => {
    albumTabsRef.current = albumTabs;
  }, [albumTabs]);

  useEffect(() => {
    void loadInitialState();
  }, []);

  useEffect(() => {
    if (loginStatus.profile_url) {
      void loadProfile(false);
    }
  }, [loginStatus.profile_url]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (disposed || event.payload.type !== "drop" || !isAlbumTabKey(activeTabRef.current)) {
          return;
        }
        void uploadFiles(event.payload.paths.filter(isImagePath), activeTabRef.current);
      })
      .then((value) => {
        unlisten = value;
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  // loadInitialState 读取设置和当前登录状态。
  async function loadInitialState() {
    try {
      const [nextSettings, status] = await Promise.all([getSettings(), getImgbbLoginStatus()]);
      setSettings(nextSettings);
      setAppState({ loginStatus: status });
      loginForm.setFieldsValue({
        login_subject: nextSettings.imgbb_login_subject,
        password: nextSettings.imgbb_password,
      });
      if (status.logged_in) {
        setActiveTab(ACCOUNT_TAB_KEY);
      }
    } catch (error) {
      message.error(String(error));
    }
  }

  // handleLogin 提交登录并保存凭据到设置。
  async function handleLogin(values: LoginForm) {
    setAuthLoading(true);
    try {
      const status = await loginImgbb(values.login_subject, values.password);
      setAppState({ loginStatus: status });
      setActiveTab(ACCOUNT_TAB_KEY);
      message.success("ImgBB 登录成功");
    } catch (error) {
      message.error(String(error));
    } finally {
      setAuthLoading(false);
    }
  }

  // handleLogout 退出登录并清理账号标签。
  async function handleLogout() {
    setLogoutLoading(true);
    try {
      const status = await logoutImgbb();
      setAppState({ loginStatus: status });
      setAlbums([]);
      setAlbumTabs([]);
      setActiveTab(LOGIN_TAB_KEY);
      loginForm.setFieldsValue({ password: "" });
      message.success("已退出 ImgBB");
    } catch (error) {
      message.error(String(error));
    } finally {
      setLogoutLoading(false);
    }
  }

  // loadProfile 加载当前账号的个人空间相册。
  async function loadProfile(refresh: boolean) {
    if (!loginStatus.profile_url) {
      return;
    }
    setProfileLoading(true);
    try {
      const response = await parseProfile(loginStatus.profile_url, refresh);
      setAlbums(response.data.albums);
      setProfilePage(1);
      message.success(response.cached ? "已读取账号空间缓存" : "账号空间已刷新");
    } catch (error) {
      message.error(String(error));
    } finally {
      setProfileLoading(false);
    }
  }

  // openManagedAlbum 打开或复用管理版相册标签。
  async function openManagedAlbum(item: ProfileAlbum, refresh = false) {
    const tabKey = buildAlbumTabKey(item.url);
    upsertAlbumTab(tabKey, item);
    setActiveTab(tabKey);
    try {
      const response = await parseAlbum(item.url, refresh);
      updateAlbumTab(tabKey, {
        title: response.data.title,
        url: response.data.url,
        album: response.data,
        selectedImageIds: response.data.images.map((image) => image.id),
        searchText: "",
        currentPage: 1,
        loading: false,
      });
      message.success(response.cached ? "已读取相册缓存" : "相册已刷新");
    } catch (error) {
      updateAlbumTab(tabKey, { loading: false });
      message.error(String(error));
    }
  }

  // handleCreateAlbum 创建新相册并刷新账号空间。
  async function handleCreateAlbum() {
    try {
      const values = await createForm.validateFields();
      setCreateLoading(true);
      await createImgbbAlbum(valuesToCreateAlbumInput(values));
      setCreateOpen(false);
      createForm.resetFields();
      await loadProfile(true);
      message.success("相册已创建");
    } catch (error) {
      if (!String(error).includes("outOfDate")) {
        message.error(String(error));
      }
    } finally {
      setCreateLoading(false);
    }
  }

  // confirmDeleteAlbum 弹窗确认后删除相册。
  function confirmDeleteAlbum(item: ProfileAlbum) {
    modal.confirm({
      title: "删除相册",
      content: `确认删除相册「${item.name}」？`,
      okText: "删除",
      okButtonProps: { danger: true },
      cancelText: "取消",
      onOk: async () => {
        const albumId = extractAlbumId(item.url);
        setDeletingAlbumUrls((current) => [...current, item.url]);
        try {
          await deleteImgbbAlbum(albumId);
          setAlbums((current) => current.filter((albumItem) => albumItem.url !== item.url));
          closeAlbumTab(buildAlbumTabKey(item.url));
          message.success("相册已删除");
        } catch (error) {
          message.error(String(error));
        } finally {
          setDeletingAlbumUrls((current) => current.filter((url) => url !== item.url));
        }
      },
    });
  }

  // confirmDeleteImage 弹窗确认后删除图片。
  function confirmDeleteImage(tabKey: string, image: AlbumImage) {
    modal.confirm({
      title: "删除图片",
      content: `确认删除图片「${image.filename}」？`,
      okText: "删除",
      okButtonProps: { danger: true },
      cancelText: "取消",
      onOk: async () => {
        updateAlbumTab(tabKey, (tab) => ({ deletingImageIds: [...tab.deletingImageIds, image.id] }));
        try {
          await deleteImgbbImage(image.id);
          updateAlbumTab(tabKey, (tab) => ({
            album: tab.album
              ? { ...tab.album, images: tab.album.images.filter((item) => item.id !== image.id) }
              : tab.album,
            selectedImageIds: tab.selectedImageIds.filter((id) => id !== image.id),
          }));
          message.success("图片已删除");
        } catch (error) {
          message.error(String(error));
        } finally {
          updateAlbumTab(tabKey, (tab) => ({
            deletingImageIds: tab.deletingImageIds.filter((id) => id !== image.id),
          }));
        }
      },
    });
  }

  // chooseUploadFiles 打开文件选择器并上传到当前相册标签。
  async function chooseUploadFiles(tabKey: string) {
    const selected = await open({
      multiple: true,
      filters: [{ name: "图片", extensions: IMAGE_EXTENSIONS }],
      fileAccessMode: "scoped",
    });
    const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
    await uploadFiles(paths.filter(isImagePath), tabKey);
  }

  // uploadFiles 上传一批本地图片路径。
  async function uploadFiles(paths: string[], tabKey: string) {
    const currentTab = albumTabsRef.current.find((tab) => tab.key === tabKey);
    if (!currentTab?.album || paths.length === 0) {
      return;
    }
    updateAlbumTab(tabKey, { uploadLoading: true });
    try {
      const albumId = extractAlbumId(currentTab.album.url);
      for (const path of paths) {
        await uploadImgbbAlbumImage(albumId, path);
      }
      const response = await parseAlbum(currentTab.album.url, true);
      updateAlbumTab(tabKey, {
        title: response.data.title,
        url: response.data.url,
        album: response.data,
        selectedImageIds: response.data.images.map((image) => image.id),
        loading: false,
      });
      message.success(`已上传 ${paths.length} 张图片`);
    } catch (error) {
      message.error(String(error));
    } finally {
      updateAlbumTab(tabKey, { uploadLoading: false });
    }
  }

  // upsertAlbumTab 打开或复用相册标签。
  function upsertAlbumTab(tabKey: string, item: ProfileAlbum) {
    setAlbumTabs((current) => {
      if (current.some((tab) => tab.key === tabKey)) {
        return current.map((tab) => (tab.key === tabKey ? { ...tab, title: item.name, loading: true } : tab));
      }
      return [
        ...current,
        {
          key: tabKey,
          title: item.name,
          url: item.url,
          selectedImageIds: [],
          searchText: "",
          currentPage: 1,
          loading: true,
          uploadLoading: false,
          deletingImageIds: [],
        },
      ];
    });
  }

  // updateAlbumTab 更新指定相册标签。
  function updateAlbumTab(
    tabKey: string,
    patch: Partial<ManagedAlbumTab> | ((tab: ManagedAlbumTab) => Partial<ManagedAlbumTab>),
  ) {
    setAlbumTabs((current) =>
      current.map((tab) => {
        if (tab.key !== tabKey) {
          return tab;
        }
        const nextPatch = typeof patch === "function" ? patch(tab) : patch;
        return { ...tab, ...nextPatch };
      }),
    );
  }

  // closeAlbumTab 关闭指定相册标签。
  function closeAlbumTab(tabKey: string) {
    setAlbumTabs((current) => current.filter((tab) => tab.key !== tabKey));
    if (activeTabRef.current === tabKey) {
      setActiveTab(ACCOUNT_TAB_KEY);
    }
  }

  const displaySettings = settingsToDisplaySettings(settings);
  const filteredAlbums = filterProfileAlbums(albums, profileSearch);
  const visibleAlbums = displaySettings.pagination_enabled
    ? paginateList(filteredAlbums, profilePage, displaySettings.profile_page_size)
    : filteredAlbums;
  const tabItems = [
    {
      key: LOGIN_TAB_KEY,
      label: "登录",
      closable: false,
      children: renderLoginTab(loginForm, loginStatus, authLoading, logoutLoading, handleLogin, handleLogout),
    },
    {
      key: ACCOUNT_TAB_KEY,
      label: "个人空间",
      closable: false,
      children: renderAccountTab(),
    },
    ...albumTabs.map((tab) => ({
      key: tab.key,
      label: tab.title,
      closable: true,
      children: renderManagedAlbumTab({
        tab,
        displaySettings,
        onUpdateTab: updateAlbumTab,
        onRefreshAlbum: (item) => openManagedAlbum(item, true),
        onChooseUploadFiles: chooseUploadFiles,
        onDeleteImage: confirmDeleteImage,
      }),
    })),
  ];

  return (
    <>
      <Tabs
        type="editable-card"
        hideAdd
        activeKey={activeTab}
        onChange={setActiveTab}
        onEdit={(targetKey, action) => {
          if (action === "remove") {
            closeAlbumTab(String(targetKey));
          }
        }}
        items={tabItems}
        className={parseStyles.browserTabs}
      />
      <Modal
        title="新增相册"
        open={createOpen}
        confirmLoading={createLoading}
        okText="创建"
        cancelText="取消"
        onOk={handleCreateAlbum}
        onCancel={() => setCreateOpen(false)}
      >
        <Form form={createForm} layout="vertical" initialValues={{ privacy: "public" }}>
          <Form.Item label="相册名称" name="name" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item label="描述" name="description">
            <Input.TextArea rows={3} />
          </Form.Item>
          <Form.Item label="可见性" name="privacy">
            <Radio.Group>
              <Radio.Button value="public">公开</Radio.Button>
              <Radio.Button value="private">私密</Radio.Button>
              <Radio.Button value="password">密码</Radio.Button>
            </Radio.Group>
          </Form.Item>
          <Form.Item label="访问密码" name="password">
            <Input.Password autoComplete="new-password" />
          </Form.Item>
        </Form>
      </Modal>
    </>
  );

  // renderAccountTab 渲染固定个人空间标签。
  function renderAccountTab() {
    if (!loginStatus.logged_in) {
      return (
        <Empty description="请先登录 ImgBB" image={Empty.PRESENTED_IMAGE_SIMPLE}>
          <Button type="primary" icon={<LoginOutlined />} onClick={() => setActiveTab(LOGIN_TAB_KEY)}>
            去登录
          </Button>
        </Empty>
      );
    }
    return (
      <Space direction="vertical" size={14} className={parseStyles.pageStack}>
        <div className={parseStyles.resultHeader}>
          <div className={parseStyles.resultTitle}>
            <Typography.Title level={4}>{loginStatus.login_subject ?? "ImgBB"}</Typography.Title>
            <Typography.Text type="secondary" ellipsis>
              {loginStatus.profile_url}
            </Typography.Text>
          </div>
          <Space className={parseStyles.resultActions}>
            <Input.Search
              allowClear
              value={profileSearch}
              onChange={(event) => {
                setProfileSearch(event.target.value);
                setProfilePage(1);
              }}
              placeholder="搜索相册"
              className={parseStyles.resultSearch}
            />
            <Button icon={<ReloadOutlined />} loading={profileLoading} onClick={() => loadProfile(true)}>
              刷新
            </Button>
            <Button type="primary" icon={<FolderAddOutlined />} onClick={() => setCreateOpen(true)}>
              新增相册
            </Button>
          </Space>
        </div>
        {renderMineAlbumList({
          visibleItems: visibleAlbums,
          total: filteredAlbums.length,
          displaySettings,
          profilePage,
          profileLoading,
          deletingAlbumUrls,
          onPageChange: setProfilePage,
          onOpenAlbum: openManagedAlbum,
          onDeleteAlbum: confirmDeleteAlbum,
        })}
      </Space>
    );
  }

}
