use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Condvar, Mutex};

use crate::image::{DecodedImage, decode_preview};
use purepic::ui::thumbnail::{THUMBNAIL_QUEUE_CAPACITY, is_supported_image, natural_path_compare};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

pub struct DirectoryScanResult {
    pub generation: u64,
    pub paths: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct ThumbnailTask {
    pub generation: u64,
    pub index: usize,
    pub path: PathBuf,
    pub target_size_px: u32,
}

pub struct ThumbnailResult {
    pub generation: u64,
    pub index: usize,
    pub path: PathBuf,
    pub target_size_px: u32,
    pub decoded: std::result::Result<DecodedImage, String>,
}

pub struct ThumbnailLoader {
    queue: Arc<TaskQueue>,
    pub results: Receiver<ThumbnailResult>,
}

#[derive(Default)]
struct TaskQueueState {
    pending: VecDeque<ThumbnailTask>,
    current: Option<(u64, usize, PathBuf, u32)>,
    closed: bool,
}

#[derive(Default)]
struct TaskQueue {
    state: Mutex<TaskQueueState>,
    ready: Condvar,
}

impl TaskQueue {
    fn replace_pending(&self, tasks: Vec<ThumbnailTask>) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let current = state.current.clone();
        state.pending.clear();
        state.pending.extend(
            tasks
                .into_iter()
                .filter(|task| current.as_ref() != Some(&task_identity(task)))
                .take(THUMBNAIL_QUEUE_CAPACITY),
        );
        drop(state);
        self.ready.notify_one();
    }
}

impl ThumbnailLoader {
    pub fn new(hwnd: HWND, ready_message: u32) -> Self {
        let queue = Arc::new(TaskQueue::default());
        let worker_queue = Arc::clone(&queue);
        let (result_sender, results) = channel();
        let raw_hwnd = hwnd.0 as usize;
        std::thread::spawn(move || {
            loop {
                let task = {
                    let mut state = worker_queue
                        .state
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    while state.pending.is_empty() && !state.closed {
                        state = worker_queue
                            .ready
                            .wait(state)
                            .unwrap_or_else(|error| error.into_inner());
                    }
                    if state.closed {
                        break;
                    }
                    let task = state.pending.pop_front().expect("pending task disappeared");
                    state.current = Some(task_identity(&task));
                    task
                };
                let decoded = decode_preview(&task.path, task.target_size_px, task.target_size_px)
                    .map_err(|error| error.to_string());
                let result = ThumbnailResult {
                    generation: task.generation,
                    index: task.index,
                    path: task.path,
                    target_size_px: task.target_size_px,
                    decoded,
                };
                worker_queue
                    .state
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .current = None;
                if result_sender.send(result).is_err() {
                    break;
                }
                post_ready(raw_hwnd, ready_message);
            }
        });
        Self { queue, results }
    }

    pub fn replace_pending(&self, tasks: Vec<ThumbnailTask>) {
        self.queue.replace_pending(tasks);
    }
}

impl Drop for ThumbnailLoader {
    fn drop(&mut self) {
        let mut state = self
            .queue
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.closed = true;
        state.pending.clear();
        drop(state);
        self.queue.ready.notify_one();
    }
}

fn task_identity(task: &ThumbnailTask) -> (u64, usize, PathBuf, u32) {
    (
        task.generation,
        task.index,
        task.path.clone(),
        task.target_size_px,
    )
}

#[cfg(test)]
fn test_task(index: usize) -> ThumbnailTask {
    ThumbnailTask {
        generation: 1,
        index,
        path: PathBuf::from(format!("{index}.jpg")),
        target_size_px: 88,
    }
}

pub fn spawn_directory_scan(
    hwnd: HWND,
    current_path: PathBuf,
    generation: u64,
    sender: std::sync::mpsc::Sender<DirectoryScanResult>,
    ready_message: u32,
) {
    let raw_hwnd = hwnd.0 as usize;
    std::thread::spawn(move || {
        let mut paths = enumerate_images(&current_path);
        paths.sort_by(|left, right| natural_path_compare(left, right));
        let _ = sender.send(DirectoryScanResult { generation, paths });
        post_ready(raw_hwnd, ready_message);
    });
}

fn enumerate_images(current_path: &Path) -> Vec<PathBuf> {
    let Some(directory) = current_path.parent() else {
        return current_path
            .is_file()
            .then(|| current_path.to_path_buf())
            .into_iter()
            .collect();
    };
    let Ok(entries) = std::fs::read_dir(directory) else {
        return current_path
            .is_file()
            .then(|| current_path.to_path_buf())
            .into_iter()
            .collect();
    };
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .is_some_and(|kind| kind.is_file())
                .then(|| entry.path())
        })
        .filter(|path| is_supported_image(path))
        .collect();
    if current_path.is_file() && !paths.iter().any(|path| paths_equal(path, current_path)) {
        paths.push(current_path.to_path_buf());
    }
    paths
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

fn post_ready(raw_hwnd: usize, message: u32) {
    let hwnd = HWND(raw_hwnd as *mut _);
    let _ = unsafe { PostMessageW(Some(hwnd), message, WPARAM(0), LPARAM(0)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_current_image_is_not_returned_as_a_stale_catalog_item() {
        let path = Path::new(r"Z:\definitely-missing\current.jpg");
        assert!(enumerate_images(path).is_empty());
    }

    #[test]
    fn pending_work_is_bounded_and_replaced_by_the_latest_viewport() {
        let queue = TaskQueue::default();
        queue.replace_pending((0..THUMBNAIL_QUEUE_CAPACITY + 5).map(test_task).collect());
        assert_eq!(
            queue.state.lock().unwrap().pending.len(),
            THUMBNAIL_QUEUE_CAPACITY
        );

        let running = test_task(90);
        queue.state.lock().unwrap().current = Some(task_identity(&running));
        queue.replace_pending([90, 91, 92].map(test_task).to_vec());
        let pending: Vec<_> = queue
            .state
            .lock()
            .unwrap()
            .pending
            .iter()
            .map(|task| task.index)
            .collect();
        assert_eq!(pending, [91, 92]);
    }
}
