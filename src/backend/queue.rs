use rand::seq::SliceRandom;
use std::path::PathBuf;

#[derive(Debug, Default)]
pub(crate) struct QueueState {
    tracks: Vec<PathBuf>,
    order: Vec<usize>,
    cursor: Option<usize>,
    shuffle: bool,
}

impl QueueState {
    pub(crate) fn set_single(&mut self, path: PathBuf) {
        self.replace(vec![path.clone()], path);
    }

    pub(crate) fn replace(&mut self, paths: Vec<PathBuf>, current: PathBuf) {
        let mut unique_paths = Vec::with_capacity(paths.len());

        for path in paths {
            if !unique_paths.iter().any(|existing| existing == &path) {
                unique_paths.push(path);
            }
        }

        self.tracks = unique_paths;
        self.order = (0..self.tracks.len()).collect();

        if self.shuffle {
            self.shuffle_order();
        }

        self.cursor = self
            .order
            .iter()
            .position(|index| self.tracks[*index] == current)
            .or_else(|| (!self.order.is_empty()).then_some(0));
    }

    pub(crate) fn set_shuffle(&mut self, enabled: bool) {
        if self.shuffle == enabled {
            return;
        }

        let current_path = self.current_path().cloned();

        self.shuffle = enabled;

        if enabled {
            self.shuffle_order();
        } else {
            self.order = (0..self.tracks.len()).collect();
        }

        self.cursor = current_path
            .and_then(|path| {
                self.order
                    .iter()
                    .position(|index| self.tracks[*index] == path)
            })
            .or_else(|| (!self.order.is_empty()).then_some(0));
    }

    pub(crate) fn current_path(&self) -> Option<&PathBuf> {
        let cursor = self.cursor?;
        let track_index = self.order.get(cursor).copied()?;

        self.tracks.get(track_index)
    }

    pub(crate) fn peek_next(&self) -> Option<&PathBuf> {
        let cursor = self.cursor?;
        let next_cursor = cursor.checked_add(1)?;
        let track_index = self.order.get(next_cursor).copied()?;

        self.tracks.get(track_index)
    }

    pub(crate) fn peek_previous(&self) -> Option<&PathBuf> {
        let cursor = self.cursor?;
        let previous_cursor = cursor.checked_sub(1)?;
        let track_index = self.order.get(previous_cursor).copied()?;

        self.tracks.get(track_index)
    }

    pub(crate) fn advance_next(&mut self) -> bool {
        let Some(cursor) = self.cursor else {
            return false;
        };

        let Some(next_cursor) = cursor.checked_add(1) else {
            return false;
        };

        if next_cursor >= self.order.len() {
            return false;
        }

        self.cursor = Some(next_cursor);
        true
    }

    pub(crate) fn advance_previous(&mut self) -> bool {
        let Some(cursor) = self.cursor else {
            return false;
        };

        let Some(previous_cursor) = cursor.checked_sub(1) else {
            return false;
        };

        self.cursor = Some(previous_cursor);
        true
    }

    pub(crate) fn has_next(&self) -> bool {
        let Some(cursor) = self.cursor else {
            return false;
        };

        cursor + 1 < self.order.len()
    }

    pub(crate) fn has_previous(&self) -> bool {
        self.cursor.is_some_and(|cursor| cursor > 0)
    }

    pub(crate) fn len(&self) -> usize {
        self.tracks.len()
    }

    pub(crate) fn shuffle_enabled(&self) -> bool {
        self.shuffle
    }

    fn shuffle_order(&mut self) {
        let mut rng = rand::rng();
        self.order.shuffle(&mut rng);
    }
}

#[cfg(test)]
mod tests {
    use super::QueueState;
    use std::path::PathBuf;

    fn path(value: &str) -> PathBuf {
        PathBuf::from(value)
    }

    #[test]
    fn queue_should_preserve_current_track_when_replaced() {
        let mut queue = QueueState::default();

        queue.replace(
            vec![path("a.mp3"), path("b.mp3"), path("c.mp3")],
            path("b.mp3"),
        );

        assert_eq!(queue.current_path(), Some(&path("b.mp3")));
        assert_eq!(queue.peek_next(), Some(&path("c.mp3")));
        assert_eq!(queue.peek_previous(), Some(&path("a.mp3")));
    }

    #[test]
    fn queue_should_move_to_next_track() {
        let mut queue = QueueState::default();

        queue.replace(
            vec![path("a.mp3"), path("b.mp3"), path("c.mp3")],
            path("a.mp3"),
        );

        assert!(queue.advance_next());
        assert_eq!(queue.current_path(), Some(&path("b.mp3")));

        assert!(queue.advance_next());
        assert_eq!(queue.current_path(), Some(&path("c.mp3")));

        assert!(!queue.advance_next());
    }

    #[test]
    fn empty_queue_should_have_no_navigation() {
        let mut queue = QueueState::default();

        queue.replace(Vec::new(), path("missing.mp3"));

        assert_eq!(queue.current_path(), None);
        assert_eq!(queue.peek_next(), None);
        assert_eq!(queue.peek_previous(), None);
        assert!(!queue.has_next());
        assert!(!queue.has_previous());
    }
}
