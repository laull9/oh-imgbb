import { App as AntdApp, ConfigProvider, theme } from "antd";
import "antd/dist/reset.css";
import { useEffect } from "react";
import { AppLayout } from "./components/app_layout";
import { useAppStore } from "./tools/store";
import { useAppDarkMode } from "./hooks/useAppDarkMode";

function App() {
  // 初始化系统暗黑监听
  useAppDarkMode();
  const darkMode = useAppStore((s) => s.darkMode);
  // 同步 DOM（给 CSS / tailwind / 自定义变量用）
  useEffect(() => {
    document.documentElement.dataset.theme = darkMode ? "dark" : "light";
  }, [darkMode]);

  return (
    <ConfigProvider
      theme={{
        algorithm: darkMode
          ? theme.darkAlgorithm
          : theme.defaultAlgorithm,

        token: {
          borderRadius: 6,
          colorPrimary: "#2563eb",
          fontFamily:
            "Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
        },
      }}
    >
      <AntdApp>
        <AppLayout />
      </AntdApp>
    </ConfigProvider>
  );
}

export default App;