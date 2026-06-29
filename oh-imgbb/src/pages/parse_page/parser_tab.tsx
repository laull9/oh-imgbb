import { ReloadOutlined, SearchOutlined, SnippetsOutlined } from "@ant-design/icons";
import { Button, Card, Empty, Input, Space, Switch, Tag, Typography } from "antd";
import type { SearchPing } from "../../api/types";
import styles from "../../css/parse_page.module.css";

// ParserTabViewProps 描述解析入口标签的交互参数。
export interface ParserTabViewProps {
  url: string;
  webSearchText: string;
  refresh: boolean;
  parserLoading: boolean;
  webSearchLoading: boolean;
  webSearchPingLoading: boolean;
  webSearchPing?: SearchPing;
  onUrlChange: (value: string) => void;
  onWebSearchTextChange: (value: string) => void;
  onRefreshChange: (value: boolean) => void;
  onParse: () => void;
  onWebSearch: () => void;
  onRefreshWebSearchPing: () => void;
  onImportClipboard: () => void;
}

// ParserTabView 渲染地址解析和网络搜索入口。
export function ParserTabView({
  url,
  webSearchText,
  refresh,
  parserLoading,
  webSearchLoading,
  webSearchPingLoading,
  webSearchPing,
  onUrlChange,
  onWebSearchTextChange,
  onRefreshChange,
  onParse,
  onWebSearch,
  onRefreshWebSearchPing,
  onImportClipboard,
}: ParserTabViewProps) {
  const availableChildren = webSearchPing?.children.filter((item) => item.available) ?? [];
  const pingStatusColor = webSearchPing?.available ? "success" : "error";
  const pingStatusText = webSearchPing
    ? webSearchPing.available
      ? `可用 · ${availableChildren.length || 1} 个入口`
      : "不可用"
    : "未检测";

  return (
    <Space direction="vertical" size={22} className={styles.pageStack}>
      <section className={styles.parserSection}>
        <div className={styles.sectionHeader}>
          <Typography.Title level={3}>地址解析</Typography.Title>
          <div className={styles.sectionDivider} />
        </div>
        <div className={styles.toolbar}>
          <Input
            value={url}
            onChange={(event) => onUrlChange(event.target.value)}
            onPressEnter={onParse}
            placeholder="粘贴 ImgBB 相册或个人空间地址"
            prefix={<SearchOutlined />}
            className={styles.urlInput}
          />
          <Button icon={<SnippetsOutlined />} onClick={onImportClipboard}>
            从剪切板导入
          </Button>
          <Space>
            <Typography.Text>刷新</Typography.Text>
            <Switch checked={refresh} onChange={onRefreshChange} />
          </Space>
          <Button type="primary" icon={<ReloadOutlined />} loading={parserLoading} onClick={onParse}>
            解析
          </Button>
        </div>
      </section>
      <section className={styles.parserSection}>
        <div className={styles.sectionHeader}>
          <Typography.Title level={3}>网络搜索</Typography.Title>
          <div className={styles.sectionDivider} />
        </div>
        <div className={styles.webSearchPanel}>
          <div className={styles.webSearchBar}>
            <Input
              value={webSearchText}
              onChange={(event) => onWebSearchTextChange(event.target.value)}
              onPressEnter={onWebSearch}
              placeholder="网络搜索公开相册"
              prefix={<SearchOutlined />}
              className={styles.webSearchInput}
            />
            <Button type="primary" icon={<SearchOutlined />} loading={webSearchLoading} onClick={onWebSearch}>
              搜索
            </Button>
          </div>
          <Card size="small" className={styles.pingCard}>
            <div className={styles.pingCardContent}>
              <div>
                <Typography.Text strong>网络搜索检测</Typography.Text>
                <div>
                  <Tag color={pingStatusColor}>{pingStatusText}</Tag>
                  {webSearchPing?.base_url && (
                    <Typography.Text type="secondary">{webSearchPing.base_url}</Typography.Text>
                  )}
                </div>
                {webSearchPing?.error && (
                  <Typography.Text type="secondary" className={styles.pingError}>
                    {webSearchPing.error}
                  </Typography.Text>
                )}
              </div>
              <Button icon={<ReloadOutlined />} loading={webSearchPingLoading} onClick={onRefreshWebSearchPing}>
                检测
              </Button>
            </div>
          </Card>
          <Typography.Text type="secondary" className={styles.webSearchHint}>
            如果不能进行搜索，可能是你的国家对于搜索引擎有一定限制，或者有搜索引擎防爬虫。
          </Typography.Text>
          <Typography.Text type="secondary" className={styles.webSearchHint}>
            可以尝试代理或者加速器，或者手动搜索。
          </Typography.Text>
        </div>
      </section>
      <Empty description="解析结果会在新标签页中打开" />
    </Space>
  );
}
