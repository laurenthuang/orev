use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const CHECK_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, PartialEq)]
struct Identity {
    volume: u64,
    file: u64,
}

fn identity(file: &File) -> io::Result<Identity> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata()?;
    Ok(Identity {
        volume: metadata.dev(),
        file: metadata.ino(),
    })
}

struct Record {
    path: PathBuf,
    // Holding the original handle prevents reuse of a deleted inode.
    _original: File,
    identity: Identity,
}

impl Record {
    fn new(path: PathBuf, original: File) -> io::Result<Self> {
        let identity = identity(&original)?;
        Ok(Self {
            path,
            _original: original,
            identity,
        })
    }

    fn verify(&self) -> io::Result<()> {
        let current = {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::symlink_metadata(&self.path)?;
            Identity {
                volume: metadata.dev(),
                file: metadata.ino(),
            }
        };
        if current != self.identity {
            return Err(io::Error::other(
                "path was replaced with a different object",
            ));
        }
        Ok(())
    }
}

fn verify(records: &[Record]) -> io::Result<()> {
    for record in records {
        record.verify().map_err(|error| {
            io::Error::other(format!(
                "run record integrity lost at `{}`: {error}",
                record.path.display()
            ))
        })?;
    }
    Ok(())
}

pub(super) struct Monitor {
    stop: Sender<()>,
    thread: Option<JoinHandle<io::Result<()>>>,
}

impl Monitor {
    pub(super) fn start(directory: &Path, metadata: &File, log: &File) -> io::Result<Self> {
        let records = vec![
            Record::new(directory.to_owned(), File::open(directory)?)?,
            Record::new(directory.join("metadata.yaml"), metadata.try_clone()?)?,
            Record::new(directory.join("output.txt"), log.try_clone()?)?,
        ];
        verify(&records)?;
        let (stop, receiver) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("orev-record-integrity".into())
            .spawn(move || {
                loop {
                    if let Err(error) = verify(&records) {
                        eprintln!(
                            "Warning: {error}; tool execution continues, but recording has failed"
                        );
                        return Err(error);
                    }
                    match receiver.recv_timeout(CHECK_INTERVAL) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                            let result = verify(&records);
                            if let Err(error) = &result {
                                eprintln!("Warning: {error}; recording has failed");
                            }
                            return result;
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                }
            })?;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }

    pub(super) fn finish(mut self) -> io::Result<()> {
        self.join()
    }

    fn join(&mut self) -> io::Result<()> {
        let _ = self.stop.send(());
        match self.thread.take() {
            Some(thread) => thread
                .join()
                .map_err(|_| io::Error::other("run record integrity monitor panicked"))?,
            None => Ok(()),
        }
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        if self.thread.is_some()
            && let Err(error) = self.join()
        {
            eprintln!("Warning: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_records_pass_and_replaced_paths_fail() {
        for replaced in ["output.txt", "metadata.yaml", "directory"] {
            let parent = tempfile::tempdir().unwrap();
            let directory = parent.path().join("records");
            fs::create_dir(&directory).unwrap();
            let metadata = File::create(directory.join("metadata.yaml")).unwrap();
            let log = File::create(directory.join("output.txt")).unwrap();
            Monitor::start(&directory, &metadata, &log)
                .unwrap()
                .finish()
                .unwrap();
            let monitor = Monitor::start(&directory, &metadata, &log).unwrap();
            if replaced == "directory" {
                fs::rename(&directory, parent.path().join("moved")).unwrap();
                fs::create_dir(&directory).unwrap();
            } else {
                let path = directory.join(replaced);
                fs::rename(&path, directory.join("moved")).unwrap();
                fs::write(path, b"replacement").unwrap();
            }
            assert!(
                monitor
                    .finish()
                    .unwrap_err()
                    .to_string()
                    .contains("integrity lost")
            );
        }
    }

    #[test]
    fn missing_records_fail_without_recreating_them() {
        for removed in ["metadata.yaml", "output.txt", "directory"] {
            let parent = tempfile::tempdir().unwrap();
            let directory = parent.path().join("records");
            fs::create_dir(&directory).unwrap();
            let metadata = File::create(directory.join("metadata.yaml")).unwrap();
            let log = File::create(directory.join("output.txt")).unwrap();
            let monitor = Monitor::start(&directory, &metadata, &log).unwrap();
            let path = if removed == "directory" {
                fs::remove_dir_all(&directory).unwrap();
                directory.clone()
            } else {
                let path = directory.join(removed);
                fs::remove_file(&path).unwrap();
                path
            };
            assert!(monitor.finish().is_err());
            assert!(!path.exists());
        }
    }

    #[test]
    fn a_detected_failure_is_not_cleared_when_the_record_returns() {
        let parent = tempfile::tempdir().unwrap();
        let directory = parent.path().join("records");
        fs::create_dir(&directory).unwrap();
        let metadata = File::create(directory.join("metadata.yaml")).unwrap();
        let log = File::create(directory.join("output.txt")).unwrap();
        let monitor = Monitor::start(&directory, &metadata, &log).unwrap();
        let moved = parent.path().join("moved");
        let path = directory.join("output.txt");
        fs::rename(&path, &moved).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !monitor.thread.as_ref().unwrap().is_finished() {
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(Duration::from_millis(10));
        }
        fs::rename(moved, &path).unwrap();
        assert!(monitor.finish().is_err());
    }
}
