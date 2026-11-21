//! Application-local recovery generations and recent project paths.
use clef_model::{
    Result,
    project::{Project, atomic_write},
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsStr,
    fs::{self, DirBuilder, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const GENERATIONS: usize = 3;
const RECENT_LIMIT: usize = 10;
const METADATA_LIMIT: u64 = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryEntry {
    pub id: String,
    pub original: Option<PathBuf>,
    pub checkpoint: Option<SystemTime>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecentStatus {
    Ready,
    ReadOnly,
    Missing,
    Unavailable(String),
}
impl RecentStatus {
    pub fn can_open(&self) -> bool {
        matches!(self, Self::Ready | Self::ReadOnly)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentProject {
    pub path: PathBuf,
    pub last_used: SystemTime,
    pub status: RecentStatus,
}

#[derive(Deserialize, Serialize)]
struct RecentRecord {
    path: Vec<u8>,
    seconds: u64,
}

fn now() -> Result<std::time::Duration> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?)
}

fn lock(path: &Path, create: bool) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(create)
        .mode(0o600)
        .open(path)?;
    file.try_lock()?;
    Ok(file)
}

fn read_metadata(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(METADATA_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > METADATA_LIMIT {
        return Err("project history metadata exceeds its size limit".into());
    }
    Ok(bytes)
}

pub(super) fn probe(path: &Path) -> RecentStatus {
    match fs::metadata(path) {
        Err(e) if e.kind() == ErrorKind::NotFound => RecentStatus::Missing,
        Err(e) => RecentStatus::Unavailable(e.to_string()),
        Ok(m) if !m.is_file() => RecentStatus::Unavailable("Not a regular project file.".into()),
        Ok(_) => match Project::read(path) {
            Ok(_) if read_only(path) => RecentStatus::ReadOnly,
            Ok(_) => RecentStatus::Ready,
            Err(e) => RecentStatus::Unavailable(e.to_string()),
        },
    }
}

pub(super) fn read_only(path: &Path) -> bool {
    OpenOptions::new().write(true).open(path).is_err()
}

pub(super) enum RecentChange<'a> {
    Use(&'a Path),
    Remove(&'a Path),
    Relink(&'a Path, &'a Path),
}

pub(super) struct Store(pub PathBuf);
impl Store {
    fn records(&self) -> Result<Vec<RecentRecord>> {
        let path = self.0.join("recent-projects.json");
        let bytes = match read_metadata(&path) {
            Ok(bytes) => bytes,
            Err(e)
                if e.downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == ErrorKind::NotFound) =>
            {
                return Ok(Vec::new());
            }
            Err(e) => return Err(e),
        };
        let records: Vec<RecentRecord> = serde_json::from_slice(&bytes)?;
        if records.len() > RECENT_LIMIT
            || records.iter().any(|r| {
                !Path::new(OsStr::from_bytes(&r.path)).is_absolute()
                    || UNIX_EPOCH
                        .checked_add(std::time::Duration::from_secs(r.seconds))
                        .is_none()
            })
        {
            return Err("invalid recent project history".into());
        }
        Ok(records)
    }

    pub(super) fn recents(&self) -> Result<Vec<RecentProject>> {
        Ok(Self::views(self.records()?, &[], None))
    }

    fn views(
        records: Vec<RecentRecord>,
        known: &[RecentProject],
        changed: Option<(&Path, RecentStatus)>,
    ) -> Vec<RecentProject> {
        records
            .into_iter()
            .map(|record| {
                let path = PathBuf::from(OsStr::from_bytes(&record.path));
                let status = changed
                    .as_ref()
                    .filter(|(updated, _)| *updated == path)
                    .map(|(_, status)| status.clone())
                    .or_else(|| {
                        known
                            .iter()
                            .find(|entry| entry.path == path)
                            .map(|entry| entry.status.clone())
                    })
                    .unwrap_or_else(|| probe(&path));
                RecentProject {
                    status,
                    path,
                    last_used: UNIX_EPOCH + std::time::Duration::from_secs(record.seconds),
                }
            })
            .collect()
    }

    pub(super) fn update_recents(
        &self,
        change: RecentChange<'_>,
        known: &[RecentProject],
    ) -> Result<Vec<RecentProject>> {
        fs::create_dir_all(&self.0)?;
        let (remove, add) = match change {
            RecentChange::Use(path) => {
                let path = path.canonicalize()?;
                let status = if read_only(&path) {
                    RecentStatus::ReadOnly
                } else {
                    RecentStatus::Ready
                };
                (None, Some((path, status)))
            }
            RecentChange::Remove(path) => (Some(path), None),
            RecentChange::Relink(old, new) => {
                let new = new.canonicalize()?;
                let status = probe(&new);
                if !status.can_open() {
                    return Err(format!("Cannot locate project: {status:?}").into());
                }
                (Some(old), Some((new, status)))
            }
        };
        let _lock = lock(&self.0.join("recent-projects.lock"), true)?;
        let mut records = self.records()?;
        records.retain(|r| {
            let previous = Path::new(OsStr::from_bytes(&r.path));
            Some(previous) != remove
                && Some(previous) != add.as_ref().map(|(path, _)| path.as_path())
        });
        if let Some((path, _)) = &add {
            records.insert(
                0,
                RecentRecord {
                    path: path.as_os_str().as_bytes().to_vec(),
                    seconds: now()?.as_secs(),
                },
            );
        }
        records.truncate(RECENT_LIMIT);
        let contents = serde_json::to_vec(&records)?;
        if contents.len() as u64 > METADATA_LIMIT {
            return Err("recent project history exceeds its size limit".into());
        }
        atomic_write(&self.0.join("recent-projects.json"), true, |file| {
            file.write_all(&contents)?;
            Ok(())
        })?;
        drop(_lock);
        Ok(Self::views(
            records,
            known,
            add.as_ref()
                .map(|(path, status)| (path.as_path(), status.clone())),
        ))
    }

    fn root(&self) -> PathBuf {
        self.0.join("Recovery")
    }

    pub(super) fn create(&self, original: Option<&Path>) -> Result<Journal> {
        let root = self.root();
        fs::create_dir_all(&root)?;
        let id = format!("{}-{}", now()?.as_nanos(), std::process::id());
        let staging = root.join(format!(".new-{id}"));
        let path = root.join(&id);
        DirBuilder::new().mode(0o700).create(&staging)?;
        let result = (|| {
            let owner = lock(&staging.join("owner.lock"), true)?;
            if let Some(original) = original {
                atomic_write(&staging.join("original"), false, |file| {
                    file.write_all(original.as_os_str().as_bytes())?;
                    Ok(())
                })?;
            }
            fs::rename(&staging, &path)?;
            Ok(Journal { path, owner })
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(staging);
        }
        result
    }

    pub(super) fn acquire(&self, id: &str) -> Result<Journal> {
        if !valid_id(id) {
            return Err("invalid recovery identifier".into());
        }
        let path = self.root().join(id);
        let owner = lock(&path.join("owner.lock"), false).map_err(|e| {
            format!("Recovery is unavailable or open in another Clef instance: {e}")
        })?;
        Ok(Journal { path, owner })
    }

    pub(super) fn recoveries(&self) -> Result<Vec<RecoveryEntry>> {
        let entries = match fs::read_dir(self.root()) {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut result = Vec::new();
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            let Some(id) = name.to_str().filter(|id| valid_id(id)) else {
                continue;
            };
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let Ok(journal) = self.acquire(id) else {
                continue;
            };
            let generations = match generations(&journal.path) {
                Ok(generations) => generations,
                Err(error) => {
                    result.push(RecoveryEntry {
                        id: id.into(),
                        original: journal.original(),
                        checkpoint: None,
                        error: Some(format!("Cannot read recovery copies: {error}")),
                    });
                    continue;
                }
            };
            if generations.is_empty() {
                let _ = journal.retire();
                continue;
            }
            let (_, checkpoint, mut error) = newest_usable(&generations);
            if checkpoint.is_some()
                && generations.len() > GENERATIONS
                && let Err(cleanup) = trim_interrupted_rotation(&generations)
            {
                error.get_or_insert_with(|| {
                    format!("Could not remove older recovery copies: {cleanup}")
                });
            }
            result.push(RecoveryEntry {
                id: id.into(),
                original: journal.original(),
                checkpoint,
                error,
            });
        }
        result.sort_by(|a, b| {
            b.checkpoint
                .cmp(&a.checkpoint)
                .then_with(|| b.id.cmp(&a.id))
        });
        Ok(result)
    }
}

fn valid_id(id: &str) -> bool {
    id.split_once('-').is_some_and(|(first, second)| {
        first.parse::<u128>().is_ok() && second.parse::<u32>().is_ok()
    })
}

pub(super) struct Journal {
    pub path: PathBuf,
    owner: File,
}
impl Drop for Journal {
    fn drop(&mut self) {
        // Release immediately even if a concurrently spawning plugin worker briefly inherited the fd.
        let _ = self.owner.unlock();
    }
}
impl Journal {
    pub(super) fn original(&self) -> Option<PathBuf> {
        let bytes = read_metadata(&self.path.join("original")).ok()?;
        let path = PathBuf::from(OsStr::from_bytes(&bytes));
        path.is_absolute().then_some(path)
    }

    pub(super) fn recover(&self) -> Result<Project> {
        let (project, _, error) = newest_usable(&generations(&self.path)?);
        project.ok_or_else(|| {
            error
                .unwrap_or_else(|| "No recovery checkpoint found.".into())
                .into()
        })
    }

    pub(super) fn retire(&self) -> Result<()> {
        let name = self.path.file_name().ok_or("invalid recovery directory")?;
        let retired = self
            .path
            .with_file_name(format!(".retired-{}", name.to_string_lossy()));
        fs::rename(&self.path, &retired)?;
        // The rename commits retirement. A crash during cleanup cannot offer a partial journal.
        let _ = fs::remove_dir_all(retired);
        Ok(())
    }
}

fn generations(dir: &Path) -> Result<Vec<(u64, PathBuf)>> {
    let mut result = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension() == Some(OsStr::new("clef"))
            && let Some(number) = path
                .file_stem()
                .and_then(OsStr::to_str)
                .and_then(|name| name.parse::<u64>().ok())
            && entry.file_type()?.is_file()
        {
            result.push((number, path));
        }
    }
    result.sort_unstable_by_key(|entry| std::cmp::Reverse(entry.0));
    Ok(result)
}

fn newest_usable(
    generations: &[(u64, PathBuf)],
) -> (Option<Project>, Option<SystemTime>, Option<String>) {
    let mut error = None;
    for (_, path) in generations {
        match Project::read(path) {
            Ok(project) => {
                return (
                    Some(project),
                    fs::metadata(path).and_then(|m| m.modified()).ok(),
                    error,
                );
            }
            Err(e) => {
                error.get_or_insert_with(|| format!("Latest checkpoint cannot be read: {e}"));
            }
        }
    }
    (None, None, error)
}

fn trim_interrupted_rotation(generations: &[(u64, PathBuf)]) -> Result<()> {
    // A crash can occur after publish but before pruning. Validate the survivors before
    // removing any excess, so corruption cannot push a usable fallback out of the window.
    let mut kept = 0;
    for (_, path) in generations {
        if kept < GENERATIONS && Project::read(path).is_ok() {
            kept += 1;
        } else {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

pub(super) fn checkpoint(directory: &Path, project: &Project) -> Result<()> {
    let previous = generations(directory)?;
    let number = previous
        .first()
        .map_or(Some(1), |(n, _)| n.checked_add(1))
        .ok_or("recovery generation exhausted")?;
    project.save(&directory.join(format!("{number}.clef")), false)?;
    // The journal has one writer. Any remaining temporary sibling predates this checkpoint.
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_name().as_bytes().starts_with(b".clef-save-") && entry.file_type()?.is_file()
        {
            fs::remove_file(entry.path())?;
        }
    }
    for (_, path) in previous.into_iter().skip(GENERATIONS - 1) {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clef_model::{
        midi::{Event, Message, Mode},
        project::{TemporaryDirectory, Track},
    };
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };

    fn recorded_project() -> Project {
        let mut project = Project {
            tracks: vec![Track {
                colour: Some(clef_model::project::TRACK_COLOURS[0]),
                id: 1,
                name: "Lead".into(),
                plugin: None,
                instrument_bypassed: false,
                instrument_offline: false,
                record_mode: Default::default(),
                mix: Default::default(),
                inserts: Vec::new(),
                take_lanes: Default::default(),
                kind: Default::default(),
                output: None,
                sends: Vec::new(),
                state: Default::default(),
            }],
            next_id: 2,
            ..Default::default()
        };
        let event = Event {
            tick: 5,
            order: 0,
            message: Message::Note {
                channel: 0,
                key: 60,
                velocity: 90,
                end: 25,
                release: 0,
                release_order: 1,
            },
        };
        assert!(
            Arc::make_mut(&mut project.midi)
                .record(1, Mode::Merge, [0, 32], vec![vec![event]], false)
                .unwrap()
        );
        project
    }

    #[test]
    fn recovery_rotates_after_success_and_falls_back_after_damage() {
        let temp = TemporaryDirectory::new("t07-rotation").unwrap();
        let store = Store(temp.0.clone());
        let journal = store.create(None).unwrap();
        let mut project = recorded_project();
        for index in 1..=4 {
            project.tracks[0].name = format!("Take {index}");
            checkpoint(&journal.path, &project).unwrap();
        }
        assert_eq!(
            generations(&journal.path)
                .unwrap()
                .iter()
                .map(|(n, _)| *n)
                .collect::<Vec<_>>(),
            [4, 3, 2]
        );
        let mut invalid = project.clone();
        invalid.next_id = 1;
        assert!(checkpoint(&journal.path, &invalid).is_err());
        let blocked_destination = journal.path.join("5.clef");
        fs::create_dir(&blocked_destination).unwrap();
        assert!(checkpoint(&journal.path, &project).is_err());
        fs::remove_dir(blocked_destination).unwrap();
        assert_eq!(journal.recover().unwrap(), project);
        assert!(
            store.recoveries().unwrap().is_empty(),
            "live sessions are excluded"
        );
        fs::write(journal.path.join("4.clef"), b"damaged").unwrap();
        let path = journal.path.clone();
        let id = path.file_name().unwrap().to_str().unwrap();
        drop(journal);
        let entries = store.recoveries().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].checkpoint.is_some());
        assert!(entries[0].error.is_some());
        let journal = store.acquire(id).unwrap();
        assert_eq!(journal.recover().unwrap().tracks[0].name, "Take 3");
        fs::write(path.join("3.clef"), b"damaged").unwrap();
        fs::write(path.join("2.clef"), b"damaged").unwrap();
        assert!(journal.recover().is_err());
        drop(journal);
        let entries = store.recoveries().unwrap();
        assert_eq!(entries.len(), 1, "all damaged generations must be listed");
        assert!(entries[0].checkpoint.is_none());
        store.acquire(id).unwrap().retire().unwrap();
        assert!(store.recoveries().unwrap().is_empty());
    }

    #[test]
    fn recent_projects_persist_order_cap_and_repair_states() {
        use std::os::unix::fs::PermissionsExt;
        let temp = TemporaryDirectory::new("t07-recent").unwrap();
        let root = temp.0.canonicalize().unwrap();
        let store = Store(root.clone());
        let mut entries = Vec::new();
        let paths: Vec<_> = (0..11)
            .map(|index| root.join(format!("{index}.clef")))
            .collect();
        for path in &paths {
            Project::default().save(path, false).unwrap();
            entries = store
                .update_recents(RecentChange::Use(path), &entries)
                .unwrap();
        }
        entries = store
            .update_recents(RecentChange::Use(&paths[1]), &entries)
            .unwrap();
        assert_eq!(entries.len(), 10);
        assert_eq!(entries[0].path, paths[1]);
        assert_eq!(entries[1].path, paths[10]);
        assert!(!entries.iter().any(|e| e.path == paths[0]));
        let moved = root.join("moved.clef");
        fs::rename(&paths[1], &moved).unwrap();
        fs::write(&paths[10], b"incompatible").unwrap();
        fs::set_permissions(&paths[9], fs::Permissions::from_mode(0o400)).unwrap();
        entries = store.recents().unwrap();
        assert_eq!(entries[0].status, RecentStatus::Missing);
        assert!(matches!(entries[1].status, RecentStatus::Unavailable(_)));
        assert_eq!(entries[2].status, RecentStatus::ReadOnly);
        assert!(
            store
                .update_recents(RecentChange::Relink(&paths[1], &paths[10]), &entries)
                .is_err()
        );
        let entries = store
            .update_recents(RecentChange::Relink(&paths[1], &moved), &entries)
            .unwrap();
        assert_eq!(entries[0].path, moved);
        let entries = store
            .update_recents(RecentChange::Remove(&paths[10]), &entries)
            .unwrap();
        assert_eq!(entries.len(), 9);
        assert_eq!(store.recents().unwrap(), entries);
    }

    #[test]
    fn interrupted_rotation_retains_three_usable_copies() {
        let directory = TemporaryDirectory::new("t07-overflow").unwrap();
        let store = Store(directory.0.clone());
        let journal = store.create(None).unwrap();
        for number in 1..=3 {
            checkpoint(&journal.path, &recorded_project()).unwrap();
            assert!(journal.path.join(format!("{number}.clef")).is_file());
        }
        fs::write(journal.path.join("4.clef"), b"damaged").unwrap();
        let path = journal.path.clone();
        drop(journal);
        let entries = store.recoveries().unwrap();
        assert!(entries[0].checkpoint.is_some() && entries[0].error.is_some());
        let numbers: Vec<_> = generations(&path)
            .unwrap()
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert_eq!(numbers, [3, 2, 1]);
    }

    #[test]
    fn checkpoint_process_child() {
        let Some(directory) = std::env::var_os("CLEF_T07_CRASH_TEST_DIR") else {
            return;
        };
        let root = PathBuf::from(directory);
        let store = Store(root.clone());
        let original = root.join("original.clef");
        let journal = store
            .create(original.is_file().then_some(original.as_path()))
            .unwrap();
        checkpoint(&journal.path, &recorded_project()).unwrap();
        let _result = atomic_write(&journal.path.join("2.clef"), false, |file| {
            file.write_all(b"partially written checkpoint")?;
            file.sync_all()?;
            fs::write(root.join("checkpoint-in-flight"), b"ready")?;
            loop {
                std::thread::park();
            }
        });
    }

    #[test]
    fn process_termination_during_publication_recovers_named_and_untitled_takes() {
        for named in [false, true] {
            let temp = TemporaryDirectory::new("t07-process").unwrap();
            let store = Store(temp.0.clone());
            let original = temp.0.join("original.clef");
            if named {
                Project::default().save(&original, false).unwrap();
            }
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "recovery::tests::checkpoint_process_child"])
                .env("CLEF_T07_CRASH_TEST_DIR", &temp.0)
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap();
            let marker = temp.0.join("checkpoint-in-flight");
            let deadline = Instant::now() + Duration::from_secs(5);
            while !marker.is_file()
                && Instant::now() < deadline
                && child.try_wait().unwrap().is_none()
            {
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(
                child.kill().is_ok(),
                "child exited before the forced termination"
            );
            assert!(!child.wait().unwrap().success());
            assert!(
                marker.is_file(),
                "never reached the partial checkpoint write"
            );
            let entries = store.recoveries().unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].original.as_ref(), named.then_some(&original));
            assert_eq!(
                store.acquire(&entries[0].id).unwrap().recover().unwrap(),
                recorded_project()
            );
            if named {
                assert_eq!(Project::read(&original).unwrap(), Project::default());
            }
        }
    }
}
