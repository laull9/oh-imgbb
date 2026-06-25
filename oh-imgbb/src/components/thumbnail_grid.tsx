import { Checkbox, Space, Spin, Typography } from "antd";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import type { AlbumImage } from "../api/types";
import { ImageDetailViewer } from "./image_detail_viewer";
import styles from "../css/thumbnail_grid.module.css";

interface ThumbnailGridProps {
  images: AlbumImage[];
  selectedIds: string[];
  onSelectedIdsChange: (ids: string[]) => void;
}

interface ThumbnailImageProps {
  imageId: string;
  src?: string;
  alt: string;
  onOpen: () => void;
}

// ThumbnailGrid 展示相册缩略图并维护选择状态。
export function ThumbnailGrid({
  images,
  selectedIds,
  onSelectedIdsChange,
}: ThumbnailGridProps) {
  const selectedSet = new Set(selectedIds);
  const [detailIndex, setDetailIndex] = useState<number>();

  return (
    <>
      <div className={styles.grid}>
        {images.map((image, index) => {
          const checked = selectedSet.has(image.id);
          const previewUrl = image.local_thumbnail_path
            ? convertFileSrc(image.local_thumbnail_path)
            : image.thumbnail_url || image.image_url;

          return (
            <div className={styles.tile} key={image.id}>
              <ThumbnailImage
                imageId={image.id}
                src={previewUrl}
                alt={image.filename}
                onOpen={() => setDetailIndex(index)}
              />
              <Space className={styles.meta} align="start">
                <Checkbox
                  checked={checked}
                  onChange={(event) => {
                    if (event.target.checked) {
                      onSelectedIdsChange([...selectedSet, image.id]);
                      return;
                    }

                    onSelectedIdsChange(selectedIds.filter((id) => id !== image.id));
                  }}
                />
                <Typography.Text ellipsis title={image.filename}>
                  {image.filename}
                </Typography.Text>
              </Space>
            </div>
          );
        })}
      </div>
      <ImageDetailViewer
        images={images}
        currentIndex={detailIndex}
        onIndexChange={setDetailIndex}
        onClose={() => setDetailIndex(undefined)}
      />
    </>
  );
}

// ThumbnailImage 在图片源变化时先展示加载态，避免分页时残留旧图。
function ThumbnailImage({ imageId, src, alt, onOpen }: ThumbnailImageProps) {
  const [displaySrc, setDisplaySrc] = useState<string>();
  const [failedSrc, setFailedSrc] = useState<string>();
  const displaySrcRef = useRef<string | undefined>(undefined);
  const imageIdRef = useRef(imageId);
  const loading = Boolean(src) && !displaySrc && failedSrc !== src;
  const failed = Boolean(src) && !displaySrc && failedSrc === src;

  useEffect(() => {
    displaySrcRef.current = displaySrc;
  }, [displaySrc]);

  useEffect(() => {
    let active = true;
    const image_changed = imageIdRef.current !== imageId;
    imageIdRef.current = imageId;
    setFailedSrc(undefined);

    if (!src) {
      setDisplaySrc(undefined);
      return () => {
        active = false;
      };
    }

    if (!image_changed && displaySrcRef.current) {
      return () => {
        active = false;
      };
    }

    setDisplaySrc(undefined);

    const image = new window.Image();
    image.decoding = "async";
    image.onload = () => {
      const decodeTask = image.decode ? image.decode() : Promise.resolve();
      decodeTask
        .catch(() => undefined)
        .then(() => {
          if (active) {
            setDisplaySrc(src);
          }
        });
    };
    image.onerror = () => {
      if (active) {
        setFailedSrc(src);
      }
    };
    image.src = src;

    return () => {
      active = false;
    };
  }, [src]);

  return (
    <button
      type="button"
      className={`${styles.image} ${styles.imageButton}`}
      onClick={onOpen}
      aria-label={`查看 ${alt}`}
    >
      {displaySrc && <img src={displaySrc} alt={alt} />}
      {(loading || (!src && !displaySrc)) && (
        <div className={styles.loading}>
          <Spin />
          <Typography.Text type="secondary">加载图片中</Typography.Text>
        </div>
      )}
      {failed && (
        <div className={styles.loading}>
          <Typography.Text type="secondary">图片加载失败</Typography.Text>
        </div>
      )}
    </button>
  );
}
