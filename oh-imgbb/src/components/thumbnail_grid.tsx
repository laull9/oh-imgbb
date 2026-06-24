import { Checkbox, Image, Space, Typography } from "antd";
import type { AlbumImage } from "../api/types";

interface ThumbnailGridProps {
  images: AlbumImage[];
  selectedIds: string[];
  onSelectedIdsChange: (ids: string[]) => void;
}

export function ThumbnailGrid({
  images,
  selectedIds,
  onSelectedIdsChange,
}: ThumbnailGridProps) {
  const selectedSet = new Set(selectedIds);

  return (
    <div className="thumbnail-grid">
      {images.map((image) => {
        const checked = selectedSet.has(image.id);
        const previewUrl = image.thumbnail_url || image.image_url;

        return (
          <div className="thumbnail-tile" key={image.id}>
            <div className="thumbnail-image">
              <Image src={previewUrl} alt={image.filename} preview={{ src: image.image_url }} />
            </div>
            <Space className="thumbnail-meta" align="start">
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
  );
}
