import { CloseCircleOutlined, ReloadOutlined } from "@ant-design/icons";
import { listen } from "@tauri-apps/api/event";
import { App, Button, Empty, List, Progress, Space, Tag, Tooltip, Typography } from "antd";
import { useEffect, useMemo, useState } from "react";
import { cancelDownloadTask, listDownloadTasks } from "../api/tauri_client";
import type { DownloadTaskRecord, DownloadTaskStatus } from "../api/types";
import styles from "../css/downloads_page.module.css";

export function DownloadsPage() {
  const { message } = App.useApp();
  const [tasks, setTasks] = useState<DownloadTaskRecord[]>([]);
  const [loading, setLoading] = useState(false);

  async function loadTasks() {
    setLoading(true);
    try {
      setTasks(await listDownloadTasks());
    } catch (error) {
      message.error(String(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void loadTasks();
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    listen<DownloadTaskRecord>("download://task_updated", (event) => {
      if (disposed) {
        return;
      }

      setTasks((current) => upsertTask(current, event.payload));
    }).then((value) => {
      unlisten = value;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  async function handleCancel(id: number) {
    try {
      const task = await cancelDownloadTask(id);
      setTasks((current) => upsertTask(current, task));
      message.success("已取消下载任务");
    } catch (error) {
      message.error(String(error));
    }
  }

  const taskStats = useMemo(() => {
    const running = tasks.filter((task) => task.status === "running" || task.status === "pending").length;
    const completed = tasks.filter((task) => task.status === "completed").length;
    return { running, completed };
  }, [tasks]);

  return (
    <Space direction="vertical" size={16} className={styles.pageStack}>
      <div className={styles.resultHeader}>
        <div>
          <Typography.Title level={4}>下载任务</Typography.Title>
          <Typography.Text type="secondary">
            进行中 {taskStats.running} 个，已完成 {taskStats.completed} 个
          </Typography.Text>
        </div>
        <Button icon={<ReloadOutlined />} loading={loading} onClick={loadTasks}>
          刷新
        </Button>
      </div>
      <List
        className={styles.list}
        loading={loading}
        dataSource={tasks}
        locale={{ emptyText: <Empty description="暂无下载任务" /> }}
        renderItem={(task) => {
          const percent =
            task.total_items > 0 ? Math.round((task.finished_items / task.total_items) * 100) : 0;
          const cancellable = task.status === "pending" || task.status === "running";

          return (
            <List.Item
              actions={[
                <Button
                  key="cancel"
                  danger
                  type="text"
                  icon={<CloseCircleOutlined />}
                  disabled={!cancellable}
                  onClick={() => handleCancel(task.id)}
                >
                  取消
                </Button>,
              ]}
            >
              <List.Item.Meta
                title={
                  <Space wrap>
                    <Tooltip title={task.title}>
                      <Typography.Text strong className={styles.titleText}>
                        {truncateText(task.title, 18)}
                      </Typography.Text>
                    </Tooltip>
                    <StatusTag status={task.status} />
                    <Tag>{task.target_kind === "profile" ? "批量" : "相册"}</Tag>
                  </Space>
                }
                description={
                  <Space direction="vertical" size={8} className={styles.rowDetail}>
                    <Typography.Text type="secondary">{task.target_url}</Typography.Text>
                    <Progress
                      percent={percent}
                      status={progressStatus(task.status)}
                      format={() =>
                        task.total_items > 0
                          ? `${task.finished_items}/${task.total_items}`
                          : `${task.finished_items}/?`
                      }
                    />
                    <Typography.Text type={task.error_message ? "danger" : "secondary"}>
                      {task.error_message ||
                        `已下载 ${task.downloaded_files} 个文件，${formatBytes(task.bytes_written)}`}
                    </Typography.Text>
                  </Space>
                }
              />
            </List.Item>
          );
        }}
      />
    </Space>
  );
}

function upsertTask(tasks: DownloadTaskRecord[], task: DownloadTaskRecord) {
  const exists = tasks.some((item) => item.id === task.id);
  if (!exists) {
    return [task, ...tasks];
  }

  return tasks.map((item) => (item.id === task.id ? task : item));
}

function StatusTag({ status }: { status: DownloadTaskStatus }) {
  const statusMap = {
    pending: { color: "default", label: "等待中" },
    running: { color: "processing", label: "进行中" },
    completed: { color: "success", label: "已完成" },
    cancelled: { color: "warning", label: "已取消" },
    failed: { color: "error", label: "失败" },
  } as const;
  const item = statusMap[status];

  return <Tag color={item.color}>{item.label}</Tag>;
}

function progressStatus(status: DownloadTaskStatus) {
  if (status === "failed") {
    return "exception";
  }
  if (status === "completed") {
    return "success";
  }

  return "active";
}

function formatBytes(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }

  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  for (const unit of units) {
    if (value < 1024) {
      return `${value.toFixed(1)} ${unit}`;
    }
    value /= 1024;
  }

  return `${value.toFixed(1)} TB`;
}

function truncateText(value: string, maxLength: number) {
  if (value.length <= maxLength) {
    return value;
  }

  return `${value.slice(0, maxLength)}...`;
}
