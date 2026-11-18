use crate::protocol::Result;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub struct NewFile {
    destination: PathBuf,
    temporary: PathBuf,
    pub file: File,
    committed: bool,
}

impl NewFile {
    pub fn create(destination: &Path) -> Result<Self> {
        let temporary = destination.with_extension(format!("clef-{}-partial", std::process::id()));
        if destination.exists() {
            return Err("destination already exists; choose a new file".into());
        }
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        Ok(Self {
            destination: destination.to_owned(),
            temporary,
            file,
            committed: false,
        })
    }
    pub fn commit(mut self) -> Result<()> {
        self.file.sync_all()?;
        // Linking is atomic and fails if another writer creates the destination meanwhile.
        std::fs::hard_link(&self.temporary, &self.destination)?;
        self.committed = true;
        std::fs::remove_file(&self.temporary)?;
        Ok(())
    }
}

impl Drop for NewFile {
    fn drop(&mut self) {
        if !self.committed {
            let _ = std::fs::remove_file(&self.temporary);
        }
    }
}

pub fn save_state(path: &Path, state: &[u8]) -> Result<()> {
    let mut output = NewFile::create(path)?;
    output.file.write_all(state)?;
    output.commit()
}

pub fn read_state(path: &Path) -> Result<Vec<u8>> {
    let mut state = Vec::new();
    File::open(path)?
        .take(crate::protocol::MAX_STATE as u64 + 1)
        .read_to_end(&mut state)?;
    if state.len() > crate::protocol::MAX_STATE {
        return Err("state file exceeds limit".into());
    }
    Ok(state)
}
