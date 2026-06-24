import {
  HeartOutlined,
  SettingOutlined,
  SearchOutlined,
} from "@ant-design/icons";
import { Grid, Layout, Menu, Typography } from "antd";
import { useState } from "react";
import { FavoritesPage } from "../pages/favorites_page";
import { ParsePage } from "../pages/parse_page";
import { SettingsPage } from "../pages/settings_page";

const { Header, Content, Sider } = Layout;

type PageKey = "parse" | "favorites" | "settings";

export function AppLayout() {
  const [page, setPage] = useState<PageKey>("parse");
  const screens = Grid.useBreakpoint();
  const use_side_nav = Boolean(screens.md);
  const menu_items = [
    { key: "parse", icon: <SearchOutlined />, label: "解析" },
    { key: "favorites", icon: <HeartOutlined />, label: "收藏" },
    { key: "settings", icon: <SettingOutlined />, label: "设置" },
  ];
  const page_title = {
    parse: "解析与下载",
    favorites: "本地收藏",
    settings: "应用设置",
  }[page];
  const page_content = (
    <>
      {page === "parse" && <ParsePage />}
      {page === "favorites" && <FavoritesPage />}
      {page === "settings" && <SettingsPage />}
    </>
  );

  if (use_side_nav) {
    return (
      <Layout className="app-shell">
        <Sider width={216} className="app-sidebar">
          <div className="app-brand">
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
          <Header className="app-header">
            <Typography.Text strong>{page_title}</Typography.Text>
          </Header>
          <Content className="app-content">{page_content}</Content>
        </Layout>
      </Layout>
    );
  }

  return (
    <Layout className="app-shell app-shell-top">
      <Header className="app-top-nav">
        <div className="app-brand">
          <Typography.Title level={4}>oh-imgbb</Typography.Title>
        </div>
        <Menu
          mode="horizontal"
          selectedKeys={[page]}
          onClick={(item) => setPage(item.key as PageKey)}
          items={menu_items}
        />
      </Header>
      <Header className="app-header">
        <Typography.Text strong>{page_title}</Typography.Text>
      </Header>
      <Content className="app-content">{page_content}</Content>
    </Layout>
  );
}
