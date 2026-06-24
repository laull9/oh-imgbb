use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Result, ensure};

use super::utils::sanitize_path_segment;

const COUNT_PLACEHOLDER: &str = "{count}";
const SHORT_COUNT_PLACEHOLDER: &str = "{}";
const ALBUM_PLACEHOLDER: &str = "{album}";
const NAME_PLACEHOLDER: &str = "{name}";

/// AlbumFileNameMode 保存相册文件命名模式。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum AlbumFileNameMode {
    PreserveOriginal,
    CountPattern(String),
}

impl Default for AlbumFileNameMode {
    /// 创建默认的原名称保留模式。
    fn default() -> Self {
        Self::PreserveOriginal
    }
}

/// AlbumFileNameGenerator 为相册文件分配不冲突的目标路径。
pub(super) struct AlbumFileNameGenerator {
    directory: PathBuf,
    album_name: String,
    mode: AlbumFileNameMode,
    occupied_paths: HashSet<PathBuf>,
    next_count: usize,
}

impl AlbumFileNameGenerator {
    /// 创建相册文件名称生成器。
    pub(super) fn new(
        directory: PathBuf,
        album_name: &str,
        mode: AlbumFileNameMode,
    ) -> Result<Self> {
        let album_name = sanitize_path_segment(album_name);
        ensure!(!album_name.is_empty(), "相册名称为空");
        validate_name_mode(&mode)?;

        Ok(Self {
            directory,
            album_name,
            mode,
            occupied_paths: HashSet::new(),
            next_count: 1,
        })
    }

    /// 返回下一个可安全用于并发下载的目标路径。
    pub(super) fn next_path(&mut self, original_filename: &str) -> Result<PathBuf> {
        let parts = FileNameParts::parse(original_filename)?;

        match self.mode.clone() {
            AlbumFileNameMode::PreserveOriginal => self.next_preserved_path(&parts),
            AlbumFileNameMode::CountPattern(pattern) => self.next_count_path(&parts, &pattern),
        }
    }

    /// 使用原文件名生成不冲突路径。
    fn next_preserved_path(&mut self, parts: &FileNameParts) -> Result<PathBuf> {
        let mut suffix: Option<usize> = None;

        loop {
            let stem = match suffix {
                Some(value) => format!("{}_{}", parts.stem, value),
                None => parts.stem.clone(),
            };
            let path = self.directory.join(parts.file_name_with_stem(&stem));
            if self.reserve_path(&path) {
                return Ok(path);
            }

            suffix = Some(suffix.unwrap_or_default().saturating_add(1));
        }
    }

    /// 使用计数模板生成不冲突路径。
    fn next_count_path(&mut self, parts: &FileNameParts, pattern: &str) -> Result<PathBuf> {
        loop {
            let count = self.next_count;
            self.next_count = self.next_count.saturating_add(1);

            let stem = render_count_pattern(pattern, count, &self.album_name, &parts.stem);
            let stem = sanitize_path_segment(&stem);
            ensure!(!stem.is_empty(), "计数命名模板生成了空文件名");

            let path = self.directory.join(parts.file_name_with_stem(&stem));
            if self.reserve_path(&path) {
                return Ok(path);
            }
        }
    }

    /// 记录路径占用并检查磁盘和本次计划内冲突。
    fn reserve_path(&mut self, path: &PathBuf) -> bool {
        if path.exists() || self.occupied_paths.contains(path) {
            return false;
        }

        self.occupied_paths.insert(path.clone())
    }
}

/// FileNameParts 保存清理后的文件主名和扩展名。
struct FileNameParts {
    stem: String,
    extension: Option<String>,
}

impl FileNameParts {
    /// 从原始文件名解析安全的主名和扩展名。
    fn parse(original_filename: &str) -> Result<Self> {
        let safe_filename = sanitize_path_segment(original_filename);
        ensure!(!safe_filename.is_empty(), "文件名为空");

        let path = Path::new(&safe_filename);
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("file");
        let stem = if stem.is_empty() { "file" } else { stem }.to_string();
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .map(str::to_string);

        Ok(Self { stem, extension })
    }

    /// 使用指定主名拼出保持原扩展名的文件名。
    fn file_name_with_stem(&self, stem: &str) -> String {
        match &self.extension {
            Some(extension) => format!("{stem}.{extension}"),
            None => stem.to_string(),
        }
    }
}

/// 校验命名模式的模板格式。
fn validate_name_mode(mode: &AlbumFileNameMode) -> Result<()> {
    let AlbumFileNameMode::CountPattern(pattern) = mode else {
        return Ok(());
    };

    ensure!(!pattern.trim().is_empty(), "计数命名模板为空");
    ensure!(
        pattern.contains(COUNT_PLACEHOLDER) || pattern.contains(SHORT_COUNT_PLACEHOLDER),
        "计数命名模板必须包含 {{count}} 或 {{}}"
    );

    let rest = pattern
        .replace(COUNT_PLACEHOLDER, "")
        .replace(SHORT_COUNT_PLACEHOLDER, "")
        .replace(ALBUM_PLACEHOLDER, "")
        .replace(NAME_PLACEHOLDER, "");
    ensure!(
        !rest.contains('{') && !rest.contains('}'),
        "计数命名模板包含未知占位符"
    );

    Ok(())
}

/// 渲染计数命名模板。
fn render_count_pattern(pattern: &str, count: usize, album: &str, name: &str) -> String {
    let count = count.to_string();

    pattern
        .replace(COUNT_PLACEHOLDER, &count)
        .replace(SHORT_COUNT_PLACEHOLDER, &count)
        .replace(ALBUM_PLACEHOLDER, album)
        .replace(NAME_PLACEHOLDER, name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// 验证原名称模式会避开本次计划内的重名。
    #[test]
    fn preserved_mode_deduplicates_planned_names() {
        let directory = PathBuf::from("downloads");
        let mut generator = AlbumFileNameGenerator::new(
            directory.clone(),
            "demo album",
            AlbumFileNameMode::default(),
        )
        .unwrap();

        assert_eq!(
            generator.next_path("a:b.jpg").unwrap(),
            directory.join("a_b.jpg")
        );
        assert_eq!(
            generator.next_path("a:b.jpg").unwrap(),
            directory.join("a_b_1.jpg")
        );
    }

    /// 验证原名称模式会避开磁盘上已有文件。
    #[test]
    fn preserved_mode_deduplicates_existing_names() {
        let directory = temp_directory("preserved_existing");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("a_b.jpg"), b"old").unwrap();

        let mut generator = AlbumFileNameGenerator::new(
            directory.clone(),
            "demo album",
            AlbumFileNameMode::default(),
        )
        .unwrap();

        assert_eq!(
            generator.next_path("a:b.jpg").unwrap(),
            directory.join("a_b_1.jpg")
        );

        fs::remove_dir_all(directory).unwrap();
    }

    /// 验证计数模板会替换占位符并保留原扩展名。
    #[test]
    fn count_pattern_uses_placeholders_and_keeps_extension() {
        let directory = PathBuf::from("downloads");
        let mut generator = AlbumFileNameGenerator::new(
            directory.clone(),
            "demo/album",
            AlbumFileNameMode::CountPattern("{album}_{count}_{name}".to_string()),
        )
        .unwrap();

        assert_eq!(
            generator.next_path("a:b.jpg").unwrap(),
            directory.join("demo_album_1_a_b.jpg")
        );
        assert_eq!(
            generator.next_path("other.png").unwrap(),
            directory.join("demo_album_2_other.png")
        );
    }

    /// 验证计数模板遇到已有文件会继续递增计数。
    #[test]
    fn count_pattern_skips_existing_names() {
        let directory = temp_directory("count_existing");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("demo_1.jpg"), b"old").unwrap();

        let mut generator = AlbumFileNameGenerator::new(
            directory.clone(),
            "demo",
            AlbumFileNameMode::CountPattern("{album}_{count}".to_string()),
        )
        .unwrap();

        assert_eq!(
            generator.next_path("a.jpg").unwrap(),
            directory.join("demo_2.jpg")
        );

        fs::remove_dir_all(directory).unwrap();
    }

    /// 验证计数模板必须包含计数占位符。
    #[test]
    fn count_pattern_requires_count_placeholder() {
        let result = AlbumFileNameGenerator::new(
            PathBuf::from("downloads"),
            "demo",
            AlbumFileNameMode::CountPattern("{album}_{name}".to_string()),
        );

        assert!(result.is_err());
    }

    /// 构造测试专用临时目录。
    fn temp_directory(name: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        std::env::temp_dir().join(format!("llpha_imgbb_{name}_{timestamp}"))
    }
}
