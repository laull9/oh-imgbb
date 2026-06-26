import {
  DownloadOutlined,
  HeartOutlined,
  SettingOutlined,
  SearchOutlined,
  UserOutlined,
} from "@ant-design/icons";
import { Grid, Layout, Menu, Typography } from "antd";
import { useState } from "react";
import { DownloadsPage } from "../pages/downloads_page";
import { FavoritesPage } from "../pages/favorites_page";
import { MinePage } from "../pages/mine_page";
import { ParsePage } from "../pages/parse_page";
import type { ParseOpenTarget } from "../pages/parse_page";
import { SettingsPage } from "../pages/settings_page";
import styles from "../css/app_layout.module.css";

const { Header, Content, Sider } = Layout;

type PageKey = "parse" | "favorites" | "downloads" | "mine" | "settings";

export function AppLayout() {
  const [page, setPage] = useState<PageKey>("parse");
  const [openTarget, setOpenTarget] = useState<ParseOpenTarget>();
  const screens = Grid.useBreakpoint();
  const use_side_nav = Boolean(screens.md);
  const menu_items = [
    { key: "parse", icon: <SearchOutlined />, label: "解析" },
    { key: "favorites", icon: <HeartOutlined />, label: "收藏" },
    { key: "downloads", icon: <DownloadOutlined />, label: "下载" },
    { key: "mine", icon: <UserOutlined />, label: "我的" },
    { key: "settings", icon: <SettingOutlined />, label: "设置" },
  ];
  const page_content = (
    <>
      {page === "parse" && (
        <ParsePage
          openTarget={openTarget}
          onTargetHandled={(id) =>
            setOpenTarget((current) =>
              current?.id === id ? undefined : current,
            )
          }
          onOpenDownloads={() => setPage("downloads")}
        />
      )}
      {page === "favorites" && (
        <FavoritesPage
          onOpenTarget={(target) => {
            setOpenTarget({ ...target, id: Date.now() });
            setPage("parse");
          }}
        />
      )}
      {page === "downloads" && <DownloadsPage />}
      {page === "mine" && <MinePage />}
      {page === "settings" && <SettingsPage />}
    </>
  );

  if (use_side_nav) {
    return (
      <Layout className={styles.shell}>
        <Sider width={186} className={styles.sidebar}>
          <div className={styles.brand}>
            <img src="/oh-ibb.png" alt="" />
            <Typography.Title level={4}>oh-imgbb</Typography.Title>
          </div>
          <Menu
            mode="inline"
            selectedKeys={[page]}
            onClick={(item) => setPage(item.key as PageKey)}
            items={menu_items}
          />
        </Sider>
        <Layout>
          <Content className={styles.content}>{page_content}</Content>
        </Layout>
      </Layout>
    );
  }

  return (
    <Layout className={`${styles.shell} ${styles.shellTop}`}>
      <Header className={styles.topNav}>
        <div className={styles.brand}>
          <img src="/oh-ibb.png" alt="" />
          <Typography.Title level={4}>oh-imgbb</Typography.Title>
        </div>
        <Menu
          mode="horizontal"
          selectedKeys={[page]}
          onClick={(item) => setPage(item.key as PageKey)}
          items={menu_items}
        />
      </Header>
      <Content className={styles.content}>{page_content}</Content>
    </Layout>
  );
}
