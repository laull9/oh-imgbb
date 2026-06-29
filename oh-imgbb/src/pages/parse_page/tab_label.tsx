import { Space, Tag, Tooltip } from "antd";
import styles from "../../css/parse_page.module.css";
import type { ParseTab } from "./types";
import { truncateText } from "./utils";

// ParseTabLabel 渲染解析页标签标题。
export function ParseTabLabel({ tab }: { tab: ParseTab }) {
  if (tab.kind === "parser") {
    return tab.title;
  }
  const label = truncateText(tab.title, 18);

  return (
    <Space size={6}>
      <Tooltip title={tab.title}>
        <span className={styles.tabTitleText}>{label}</span>
      </Tooltip>
      {tab.kind === "profile" && <Tag>{tab.source === "search" ? "搜索" : "空间"}</Tag>}
      {tab.loading && <Tag color="processing">加载</Tag>}
    </Space>
  );
}
