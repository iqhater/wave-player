use audiotags::Tag;
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub struct TrackMetadata {
    pub artist: String,
    pub title: String,
    pub album: String,
    pub genre: String,
    // pub duration: u64,
    /// JPEG bytes for album cover, or empty Vec if no cover
    pub cover: Vec<u8>,
    pub track_path: String,
}

/// Parse metadata from audio file using audiotags.
/// Falls back to filename parsing if tags are missing.
pub fn parse_track_metadata(path: &Path) -> TrackMetadata {
    // Try audiotags first
    if let Ok(tag) = Tag::new().read_from_path(path) {
        let title = tag.title().map(|s| s.to_string()).filter(|s| !s.is_empty());
        let artist = tag
            .artist()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());

        let album = tag
            .album_title()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());

        let genre = tag.genre().map(|s| s.to_string()).filter(|s| !s.is_empty());

        let cover = tag
            .album_cover()
            .map(|pic| pic.data.to_vec())
            .unwrap_or_default();

        // If both title and artist are present from tags — use them
        if let (Some(t), Some(a)) = (title.clone(), artist.clone()) {
            return TrackMetadata {
                artist: a,
                title: t,
                album: album.unwrap_or("".to_string()),
                genre: genre.unwrap_or("".to_string()),
                // duration: duration,
                cover,
                track_path: path.to_string_lossy().into_owned(),
            };
        }

        // Fallback: use what we have + filename parsing
        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();

        let (parsed_artist, parsed_title) = parse_filename(&file_stem);

        return TrackMetadata {
            artist: artist.unwrap_or(parsed_artist),
            title: title.unwrap_or(parsed_title),
            album: album.unwrap_or("".to_string()),
            genre: genre.unwrap_or("".to_string()),
            // duration: duration,
            cover,
            track_path: path.to_string_lossy().into_owned(),
        };
    }

    // Fallback: filename-only parsing
    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let (artist, title) = parse_filename(&file_stem);

    TrackMetadata {
        artist,
        title,
        album: "".to_string(),
        genre: "".to_string(),
        // duration: 0,
        cover: vec![],
        track_path: path.to_string_lossy().into_owned(),
    }
}

/// Парсит имя файла вида "Artist - Title"
fn parse_filename(file_stem: &str) -> (String, String) {
    let separators = [" - ", " – ", " — "];

    for sep in &separators {
        if let Some(pos) = file_stem.find(sep) {
            let artist = file_stem[..pos].trim().to_string();
            let title = file_stem[pos + sep.len()..].trim().to_string();

            if !title.is_empty() {
                let artist = if artist.is_empty() {
                    "Unknown Artist".to_string()
                } else {
                    artist
                };
                return (artist, title);
            }
        }
    }

    ("Unknown Artist".to_string(), file_stem.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_artist_dash_title() {
        let path = std::path::PathBuf::from("Artist Name - Song Title.mp3");
        let meta = parse_track_metadata(&path);
        assert_eq!(meta.artist, "Artist Name");
        assert_eq!(meta.title, "Song Title");
    }

    #[test]
    fn parses_em_dash() {
        let path = std::path::PathBuf::from("Artist Name – Song Title.flac");
        let meta = parse_track_metadata(&path);
        assert_eq!(meta.artist, "Artist Name");
        assert_eq!(meta.title, "Song Title");
    }

    #[test]
    fn parses_long_em_dash() {
        let path = std::path::PathBuf::from("Artist Name — Song Title.wav");
        let meta = parse_track_metadata(&path);
        assert_eq!(meta.artist, "Artist Name");
        assert_eq!(meta.title, "Song Title");
    }

    #[test]
    fn fallback_to_unknown_artist() {
        let path = std::path::PathBuf::from("Just a song name.ogg");
        let meta = parse_track_metadata(&path);
        assert_eq!(meta.artist, "Unknown Artist");
        assert_eq!(meta.title, "Just a song name");
    }

    #[test]
    fn handles_empty_artist() {
        let path = std::path::PathBuf::from(" - only title.mp3");
        let meta = parse_track_metadata(&path);
        assert_eq!(meta.artist, "Unknown Artist");
        assert_eq!(meta.title, "only title");
    }

    #[test]
    fn trims_spaces() {
        let path = std::path::PathBuf::from("  Artist   -   Title  .mp3");
        let meta = parse_track_metadata(&path);
        assert_eq!(meta.artist, "Artist");
        assert_eq!(meta.title, "Title");
    }
}
