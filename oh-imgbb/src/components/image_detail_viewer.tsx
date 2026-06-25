import {
  CloseOutlined,
  LeftOutlined,
  MinusOutlined,
  PlusOutlined,
  ReloadOutlined,
  RightOutlined,
} from "@ant-design/icons";
import { App, Button, Spin, Tooltip, Typography } from "antd";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import { downloadDetailImage, removeDetailImage } from "../api/tauri_client";
import type { AlbumImage } from "../api/types";
import styles from "../css/image_detail_viewer.module.css";

interface ImageDetailViewerProps {
  images: AlbumImage[];
  currentIndex?: number;
  onIndexChange: (index: number) => void;
  onClose: () => void;
}

const MIN_SCALE = 0.25;
const MAX_SCALE = 4;
const SCALE_STEP = 0.25;

// ImageDetailViewer 展示详情图遮罩并处理缩放和切换。
export function ImageDetailViewer({
  images,
  currentIndex,
  onIndexChange,
  onClose,
}: ImageDetailViewerProps) {
  const { message } = App.useApp();
  const [detailPath, setDetailPath] = useState<string>();
  const [loading, setLoading] = useState(false);
  const [errorText, setErrorText] = useState<string>();
  const [scale, setScale] = useState(1);
  const [reloadKey, setReloadKey] = useState(0);
  const open = currentIndex !== undefined;
  const currentImage = open ? images[currentIndex] : undefined;
  const previewSrc = useMemo(() => {
    if (!currentImage) {
      return undefined;
    }

    return currentImage.local_thumbnail_path
      ? convertFileSrc(currentImage.local_thumbnail_path)
      : currentImage.thumbnail_url || currentImage.image_url;
  }, [currentImage]);
  const detailSrc = useMemo(
    () => (detailPath ? convertFileSrc(detailPath) : undefined),
    [detailPath],
  );

  useEffect(() => {
    if (!open || !currentImage) {
      setDetailPath(undefined);
      setErrorText(undefined);
      setLoading(false);
      return;
    }

    let disposed = false;
    setDetailPath(undefined);
    setErrorText(undefined);
    setScale(1);
    setLoading(true);

    downloadDetailImage(currentImage.image_url)
      .then((response) => {
        if (disposed) {
          void removeDetailImage(response.local_path);
          return;
        }

        setDetailPath(response.local_path);
      })
      .catch((error) => {
        if (disposed) {
          return;
        }

        const text = String(error);
        setErrorText(text);
        message.error(text);
      })
      .finally(() => {
        if (!disposed) {
          setLoading(false);
        }
      });

    return () => {
      disposed = true;
    };
  }, [open, currentImage?.id, currentImage?.image_url, reloadKey, message]);

  useEffect(() => {
    return () => {
      if (detailPath) {
        void removeDetailImage(detailPath);
      }
    };
  }, [detailPath]);

  useEffect(() => {
    if (!open) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
        return;
      }

      if (event.key === "ArrowLeft") {
        changeImage(-1);
        return;
      }

      if (event.key === "ArrowRight") {
        changeImage(1);
      }
    };

    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [open, currentIndex, images.length]);

  if (!open || !currentImage) {
    return null;
  }

  function changeImage(offset: number) {
    if (currentIndex === undefined || images.length === 0) {
      return;
    }

    onIndexChange((currentIndex + offset + images.length) % images.length);
  }

  function changeScale(offset: number) {
    setScale((value) => Math.min(MAX_SCALE, Math.max(MIN_SCALE, value + offset)));
  }

  return (
    <div className={styles.overlay} role="dialog" aria-modal="true">
      <div className={styles.topbar}>
        <Typography.Text className={styles.title} ellipsis title={currentImage.filename}>
          {currentImage.filename}
        </Typography.Text>
        <div className={styles.tools}>
          <Tooltip title="缩小">
            <Button
              shape="circle"
              icon={<MinusOutlined />}
              disabled={scale <= MIN_SCALE}
              onClick={() => changeScale(-SCALE_STEP)}
            />
          </Tooltip>
          <Typography.Text className={styles.scale}>
            {Math.round(scale * 100)}%
          </Typography.Text>
          <Tooltip title="放大">
            <Button
              shape="circle"
              icon={<PlusOutlined />}
              disabled={scale >= MAX_SCALE}
              onClick={() => changeScale(SCALE_STEP)}
            />
          </Tooltip>
          <Tooltip title="重新加载">
            <Button
              shape="circle"
              icon={<ReloadOutlined />}
              loading={loading}
              onClick={() => setReloadKey((value) => value + 1)}
            />
          </Tooltip>
          <Tooltip title="关闭">
            <Button shape="circle" icon={<CloseOutlined />} onClick={onClose} />
          </Tooltip>
        </div>
      </div>

          <Tooltip title="上一张">
        <Button
          className={`${styles.nav} ${styles.navLeft}`}
          shape="circle"
          icon={<LeftOutlined />}
          disabled={images.length <= 1}
          onClick={() => changeImage(-1)}
        />
      </Tooltip>
      <Tooltip title="下一张">
        <Button
          className={`${styles.nav} ${styles.navRight}`}
          shape="circle"
          icon={<RightOutlined />}
          disabled={images.length <= 1}
          onClick={() => changeImage(1)}
        />
      </Tooltip>

      <div className={styles.stage}>
        {previewSrc && (
          <img
            className={detailSrc && !loading ? styles.image : `${styles.image} ${styles.preview}`}
            src={detailSrc && !loading ? detailSrc : previewSrc}
            alt={currentImage.filename}
            style={{ transform: `scale(${scale})` }}
          />
        )}
        {loading && (
          <div className={styles.loading}>
            <Spin size="large" />
          </div>
        )}
        {!loading && errorText && (
          <div className={styles.state}>
            <Typography.Text type="danger">{errorText}</Typography.Text>
          </div>
        )}
        {!loading && !errorText && detailSrc && !previewSrc && (
          <img
            className={styles.image}
            src={detailSrc}
            alt={currentImage.filename}
            style={{ transform: `scale(${scale})` }}
          />
        )}
      </div>
    </div>
  );
}
