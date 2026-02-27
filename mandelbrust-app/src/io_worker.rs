use std::path::PathBuf;
use std::sync::mpsc;

use tracing::{debug, error, warn};

use crate::bookmarks::Bookmark;

/// Request sent from the UI thread to the I/O worker.
pub(crate) enum IoRequest {
    /// Write `content` (already serialised JSON) to `path`, creating parent dirs.
    WriteFile { path: PathBuf, content: String },
    /// Delete the file at `path`.
    DeleteFile { path: PathBuf },
    /// Scan `dir` for `.json` bookmark files and send the result back.
    ScanBookmarkDir { dir: PathBuf },
}

/// Response sent from the I/O worker back to the UI thread.
pub(crate) enum IoResponse {
    /// Result of a `ScanBookmarkDir` request.
    BookmarksScanComplete {
        bookmarks: Vec<Bookmark>,
        filenames: Vec<String>,
    },
}

/// Spawn a dedicated I/O worker thread.
///
/// Returns the send-side for requests and the receive-side for responses.
/// The thread runs until the request sender is dropped.
pub(crate) fn spawn_io_worker() -> (mpsc::Sender<IoRequest>, mpsc::Receiver<IoResponse>) {
    let (req_tx, req_rx) = mpsc::channel::<IoRequest>();
    let (resp_tx, resp_rx) = mpsc::channel::<IoResponse>();

    std::thread::Builder::new()
        .name("io-worker".into())
        .spawn(move || {
            debug!("IO worker thread started");
            while let Ok(request) = req_rx.recv() {
                match request {
                    IoRequest::WriteFile { path, content } => {
                        if let Some(parent) = path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Err(e) = std::fs::write(&path, &content) {
                            error!("IO worker: failed to write {}: {e}", path.display());
                        }
                    }
                    IoRequest::DeleteFile { path } => {
                        if let Err(e) = std::fs::remove_file(&path) {
                            debug!("IO worker: could not delete {}: {e}", path.display());
                        }
                    }
                    IoRequest::ScanBookmarkDir { dir } => {
                        let (bookmarks, filenames) = scan_bookmark_dir(&dir);
                        let _ = resp_tx.send(IoResponse::BookmarksScanComplete {
                            bookmarks,
                            filenames,
                        });
                    }
                }
            }
            debug!("IO worker thread exiting");
        })
        .expect("Failed to spawn IO worker thread");

    (req_tx, resp_rx)
}

fn scan_bookmark_dir(dir: &std::path::Path) -> (Vec<Bookmark>, Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            error!(
                "IO worker: failed to read bookmark dir {}: {e}",
                dir.display()
            );
            return (Vec::new(), Vec::new());
        }
    };

    let mut bookmarks = Vec::new();
    let mut filenames = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(json) => match serde_json::from_str::<Bookmark>(&json) {
                Ok(bm) => {
                    let fname = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    filenames.push(fname);
                    bookmarks.push(bm);
                }
                Err(e) => {
                    warn!("IO worker: skipping invalid bookmark {}: {e}", path.display());
                }
            },
            Err(e) => {
                warn!("IO worker: failed to read {}: {e}", path.display());
            }
        }
    }

    debug!(
        "IO worker: scanned {} bookmarks from {}",
        bookmarks.len(),
        dir.display()
    );
    (bookmarks, filenames)
}
