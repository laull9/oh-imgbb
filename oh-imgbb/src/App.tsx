import { App as AntdApp, ConfigProvider } from "antd";
import "antd/dist/reset.css";
import "./App.css";
import { AppLayout } from "./components/app_layout";

function App() {
  return (
    <ConfigProvider
      theme={{
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
