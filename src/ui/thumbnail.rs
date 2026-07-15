use std::cmp::Ordering;
use std::ops::Range;
use std::path::Path;

use crate::ui::layout::{RectF, ThumbnailDock};

pub const THUMBNAIL_CONTENT_DIP: f32 = 72.0;
pub const THUMBNAIL_ITEM_EXTENT_DIP: f32 = 84.0;
pub const THUMBNAIL_PANEL_PADDING_DIP: f32 = 8.0;
pub const THUMBNAIL_CACHE_BUDGET_BYTES: usize = 32 * 1024 * 1024;
pub const THUMBNAIL_QUEUE_CAPACITY: usize = 24;
pub const SUPPORTED_IMAGE_EXTENSIONS: &[&str] =
    &["jpg", "jpeg", "png", "bmp", "gif", "tif", "tiff", "webp"];

pub fn fit_thumbnail_overlay(
    mut available: RectF,
    dock: ThumbnailDock,
    item_count: usize,
) -> RectF {
    let desired_extent =
        item_count as f32 * THUMBNAIL_ITEM_EXTENT_DIP + THUMBNAIL_PANEL_PADDING_DIP * 2.0;
    if dock.is_horizontal() {
        let width = desired_extent.min(available.width);
        available.x += (available.width - width) * 0.5;
        available.width = width;
    } else {
        let height = desired_extent.min(available.height);
        available.y += (available.height - height) * 0.5;
        available.height = height;
    }
    available
}

pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            SUPPORTED_IMAGE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
}

pub fn natural_path_compare(left: &Path, right: &Path) -> Ordering {
    let left = left
        .file_name()
        .unwrap_or(left.as_os_str())
        .to_string_lossy()
        .to_lowercase();
    let right = right
        .file_name()
        .unwrap_or(right.as_os_str())
        .to_string_lossy()
        .to_lowercase();
    natural_compare(&left, &right).then_with(|| left.cmp(&right))
}

pub fn visible_prefetch_range(
    item_count: usize,
    scroll_offset: f32,
    viewport_extent: f32,
) -> Range<usize> {
    if item_count == 0 || viewport_extent <= 0.0 {
        return 0..0;
    }
    let first_visible = (scroll_offset.max(0.0) / THUMBNAIL_ITEM_EXTENT_DIP).floor() as usize;
    let visible_count = (viewport_extent / THUMBNAIL_ITEM_EXTENT_DIP).ceil() as usize + 1;
    let prefetch_count = (viewport_extent / THUMBNAIL_ITEM_EXTENT_DIP).ceil() as usize;
    first_visible.saturating_sub(prefetch_count)
        ..(first_visible + visible_count + prefetch_count).min(item_count)
}

pub fn prioritized_thumbnail_indices(
    item_count: usize,
    scroll_offset: f32,
    viewport_extent: f32,
    selected: Option<usize>,
) -> Vec<usize> {
    if item_count == 0 || viewport_extent <= 0.0 {
        return Vec::new();
    }

    let first_visible = (scroll_offset.max(0.0) / THUMBNAIL_ITEM_EXTENT_DIP).floor() as usize;
    let visible_count = (viewport_extent / THUMBNAIL_ITEM_EXTENT_DIP).ceil() as usize + 1;
    let visible_end = (first_visible + visible_count).min(item_count);
    let prefetch_count = (viewport_extent / THUMBNAIL_ITEM_EXTENT_DIP).ceil() as usize;
    let prefetch_start = first_visible.saturating_sub(prefetch_count);
    let prefetch_end = (visible_end + prefetch_count).min(item_count);

    let mut indices = Vec::with_capacity(prefetch_end - prefetch_start + 3);
    let mut included = vec![false; item_count];
    let mut push = |index: usize| {
        if index < item_count && !included[index] {
            included[index] = true;
            indices.push(index);
        }
    };

    if let Some(selected) = selected.filter(|index| *index < item_count) {
        push(selected);
        if selected > 0 {
            push(selected - 1);
        }
        push(selected + 1);
    }
    for index in first_visible..visible_end {
        push(index);
    }
    for index in (prefetch_start..first_visible).rev() {
        push(index);
    }
    for index in visible_end..prefetch_end {
        push(index);
    }
    indices
}

pub fn max_scroll_offset(item_count: usize, viewport_extent: f32) -> f32 {
    (item_count as f32 * THUMBNAIL_ITEM_EXTENT_DIP - viewport_extent).max(0.0)
}

pub fn centered_scroll_offset(index: usize, item_count: usize, viewport_extent: f32) -> f32 {
    let desired = (index as f32 + 0.5) * THUMBNAIL_ITEM_EXTENT_DIP - viewport_extent * 0.5;
    desired.clamp(0.0, max_scroll_offset(item_count, viewport_extent))
}

fn natural_compare(left: &str, right: &str) -> Ordering {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let (mut left_index, mut right_index) = (0, 0);
    while left_index < left.len() && right_index < right.len() {
        if left[left_index].is_ascii_digit() && right[right_index].is_ascii_digit() {
            let left_end = digit_run_end(left, left_index);
            let right_end = digit_run_end(right, right_index);
            let left_digits = trim_zeroes(&left[left_index..left_end]);
            let right_digits = trim_zeroes(&right[right_index..right_end]);
            let ordering = left_digits
                .len()
                .cmp(&right_digits.len())
                .then_with(|| left_digits.cmp(right_digits))
                .then_with(|| (left_end - left_index).cmp(&(right_end - right_index)));
            if ordering != Ordering::Equal {
                return ordering;
            }
            left_index = left_end;
            right_index = right_end;
            continue;
        }
        let ordering = left[left_index].cmp(&right[right_index]);
        if ordering != Ordering::Equal {
            return ordering;
        }
        left_index += 1;
        right_index += 1;
    }
    left.len().cmp(&right.len())
}

fn digit_run_end(value: &[u8], start: usize) -> usize {
    value[start..]
        .iter()
        .position(|character| !character.is_ascii_digit())
        .map_or(value.len(), |offset| start + offset)
}

fn trim_zeroes(value: &[u8]) -> &[u8] {
    let first_nonzero = value
        .iter()
        .position(|character| *character != b'0')
        .unwrap_or(value.len().saturating_sub(1));
    &value[first_nonzero..]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn natural_sort_orders_numeric_file_names() {
        let mut paths = [
            PathBuf::from("10.jpg"),
            PathBuf::from("2.jpg"),
            PathBuf::from("01.jpg"),
            PathBuf::from("1.jpg"),
        ];
        paths.sort_by(|left, right| natural_path_compare(left, right));
        let names: Vec<_> = paths
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["1.jpg", "01.jpg", "2.jpg", "10.jpg"]);
    }

    #[test]
    fn visible_range_prefetches_one_viewport_on_each_side() {
        assert_eq!(visible_prefetch_range(100, 840.0, 420.0), 5..21);
    }

    #[test]
    fn prioritized_indices_load_selection_and_visible_items_before_prefetch() {
        assert_eq!(
            prioritized_thumbnail_indices(100, 840.0, 420.0, Some(50)),
            [
                50, 49, 51, 10, 11, 12, 13, 14, 15, 9, 8, 7, 6, 5, 16, 17, 18, 19, 20
            ]
        );
    }

    #[test]
    fn selected_item_can_be_centered_and_clamped() {
        assert_eq!(centered_scroll_offset(0, 20, 420.0), 0.0);
        assert_eq!(centered_scroll_offset(10, 20, 420.0), 672.0);
        assert_eq!(centered_scroll_offset(19, 20, 420.0), 1260.0);
    }

    #[test]
    fn supported_extensions_are_case_insensitive() {
        assert!(is_supported_image(Path::new("photo.JPEG")));
        assert!(is_supported_image(Path::new("image.png")));
        assert!(!is_supported_image(Path::new("notes.txt")));
    }

    #[test]
    fn short_thumbnail_overlays_shrink_and_center_on_their_main_axis() {
        let available = RectF::new(0.0, 44.0, 1_280.0, 92.0);
        assert_eq!(
            fit_thumbnail_overlay(available, ThumbnailDock::Bottom, 3),
            RectF::new(506.0, 44.0, 268.0, 92.0)
        );

        let available = RectF::new(0.0, 44.0, 92.0, 712.0);
        assert_eq!(
            fit_thumbnail_overlay(available, ThumbnailDock::Left, 2),
            RectF::new(0.0, 308.0, 92.0, 184.0)
        );
    }
}
