use crate::paths::CacheHome;
use eyre::Context;
use std::fmt::Write as _;
use std::path::Path;
use std::path::PathBuf;

const SYNC_STATE_ROOT_DIR: &str = "sync";
const SYNC_TARGETS_DIR: &str = "targets";
const CHECKPOINT_FILE_NAME: &str = "checkpoint.json";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncStateLayout {
    pub root_dir: PathBuf,
    pub targets_dir: PathBuf,
    pub target_dir: PathBuf,
    pub checkpoint_path: PathBuf,
}

#[must_use]
pub fn sync_state_root(cache_home: &CacheHome) -> PathBuf {
    cache_home.0.join(SYNC_STATE_ROOT_DIR)
}

#[must_use]
pub fn sync_targets_dir(cache_home: &CacheHome) -> PathBuf {
    sync_state_root(cache_home).join(SYNC_TARGETS_DIR)
}

#[must_use]
pub fn encode_path_component(path: &Path) -> String {
    let bytes = path.as_os_str().to_string_lossy();
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes.as_bytes() {
        let _ = write!(&mut encoded, "{byte:02x}");
    }
    encoded
}

#[must_use]
pub fn sync_target_key(output_root: &Path) -> String {
    encode_path_component(output_root)
}

#[must_use]
pub fn sync_target_state_dir(cache_home: &CacheHome, output_root: &Path) -> PathBuf {
    sync_targets_dir(cache_home).join(sync_target_key(output_root))
}

#[must_use]
pub fn sync_target_checkpoint_path(cache_home: &CacheHome, output_root: &Path) -> PathBuf {
    sync_target_state_dir(cache_home, output_root).join(CHECKPOINT_FILE_NAME)
}

#[must_use]
pub fn sync_state_layout(cache_home: &CacheHome, output_root: &Path) -> SyncStateLayout {
    let root_dir = sync_state_root(cache_home);
    let targets_dir = sync_targets_dir(cache_home);
    let target_state_dir = sync_target_state_dir(cache_home, output_root);
    let checkpoint_path = sync_target_checkpoint_path(cache_home, output_root);
    SyncStateLayout {
        root_dir,
        targets_dir,
        target_dir: target_state_dir,
        checkpoint_path,
    }
}

/// # Errors
///
/// This function will return an error if the sync state directories cannot be created.
pub fn ensure_sync_state_layout(
    cache_home: &CacheHome,
    output_root: &Path,
) -> eyre::Result<SyncStateLayout> {
    let layout = sync_state_layout(cache_home, output_root);
    std::fs::create_dir_all(&layout.target_dir).wrap_err_with(|| {
        format!(
            "Failed to create sync state directory {}",
            layout.target_dir.display()
        )
    })?;
    Ok(layout)
}

#[cfg(test)]
mod tests {
    use super::ensure_sync_state_layout;
    use super::sync_state_layout;
    use crate::paths::CacheHome;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn sync_state_layout_places_state_under_cache() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let cache_home = CacheHome(temp_dir.path().join("cache"));
        let output_root = PathBuf::from("C:/archive/discord");

        let layout = sync_state_layout(&cache_home, &output_root);

        assert_eq!(layout.root_dir, cache_home.0.join("sync"));
        assert_eq!(
            layout.targets_dir,
            cache_home.0.join("sync").join("targets")
        );
        assert!(layout.target_dir.starts_with(&layout.targets_dir));
        assert_eq!(
            layout.checkpoint_path,
            layout.target_dir.join("checkpoint.json")
        );
    }

    #[test]
    fn ensure_sync_state_layout_creates_target_dir() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let cache_home = CacheHome(temp_dir.path().join("cache"));
        let output_root = temp_dir.path().join("output");

        let layout = ensure_sync_state_layout(&cache_home, &output_root)
            .expect("sync state layout should be created");

        assert!(layout.target_dir.exists());
        assert!(layout.target_dir.is_dir());
    }
}
