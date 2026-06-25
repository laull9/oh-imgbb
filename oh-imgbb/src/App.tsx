import { App as AntdApp, ConfigProvider } from "antd";
import { theme } from "antd";
import "antd/dist/reset.css";
import { useEffect, useState } from "react";
import { AppLayout } from "./components/app_layout";

const COLOR_SCHEME_QUERY = "(prefers-color-scheme: dark)";

// getSystemDarkMode 读取系统当前亮暗模式。
function getSystemDarkMode() {
  if (typeof window === "undefined" || !window.matchMedia) {
    return false;
  }

  return window.matchMedia(COLOR_SCHEME_QUERY).matches;
}

function App() {
  const [darkMode, setDarkMode] = useState(getSystemDarkMode);

  useEffect(() => {
    const media = window.matchMedia(COLOR_SCHEME_QUERY);
    const handleChange = (event: MediaQueryListEvent) => {
      setDarkMode(event.matches);
    };

    setDarkMode(media.matches);
    media.addEventListener("change", handleChange);

    return () => {
      media.removeEventListener("change", handleChange);
    };
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = darkMode ? "dark" : "light";
  }, [darkMode]);

  return (
    <ConfigProvider
      theme={{
        algorithm: darkMode ? theme.darkAlgorithm : theme.defaultAlgorithm,
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
