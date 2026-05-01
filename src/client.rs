use crate::database::extension::DownloadStatus;
use dirs::data_dir;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::Row;
use std::error::Error;

use crate::config::AuthEntry;
use crate::helpers::Searchable;
use crate::themes::dialoguer::DialogTheme;
use dialoguer::Confirm;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct TempDiscographyAlbum {
    pub id: String,
    pub songs: Vec<DiscographySong>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlaylistResult {
    pub items: Vec<DiscographySong>,
    pub total_record_count: u64,
}

#[derive(Debug)]
pub struct Client {
    pub base_url: String,
    pub server_id: String,
    http_client: reqwest::Client,
    pub user_id: String,
    pub user_name: String,
    pub token: String,
    pub salt: String,
    pub device_id: String,
}

#[derive(Debug, Clone)]
pub enum AuthMethod {
    UserPass { username: String, password: String },
}

#[derive(Debug, Clone)]
pub struct SelectedServer {
    pub url: String,
    pub auth: AuthMethod,
}

#[derive(Debug)]
pub struct Transcoding {
    pub enabled: bool,
    pub bitrate: u32,
    pub container: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkQuality {
    Normal,
    Slow,
    CzechTrain,
}

impl NetworkQuality {
    pub fn classify(ms: u128) -> Self {
        match ms {
            0..=300 => NetworkQuality::Normal,
            301..=1200 => NetworkQuality::Slow,
            _ => NetworkQuality::CzechTrain,
        }
    }
}

// Data models for Subsonic Response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubsonicResponseRoot {
    #[serde(rename = "subsonic-response")]
    response: SubsonicResponseData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubsonicResponseData {
    status: String,
    error: Option<SubsonicError>,
    music_folders: Option<MusicFolders>,
    indexes: Option<Indexes>,
    album_list2: Option<AlbumList2>,
    album: Option<SubsonicAlbum>,
    artist: Option<SubsonicArtistDetails>,
    search_result3: Option<SearchResult3>,
    random_songs: Option<RandomSongs>,
    playlists: Option<SubsonicPlaylists>,
    playlist: Option<SubsonicPlaylistWithSongs>,
    lyrics_list: Option<SubsonicLyricsList>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct SubsonicError {
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MusicFolders {
    music_folder: Vec<SubsonicFolder>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubsonicFolder {
    id: i64,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Indexes {
    index: Vec<SubsonicIndex>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubsonicIndex {
    artist: Vec<SubsonicArtist>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SubsonicArtist {
    id: String,
    name: String,
    starred: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AlbumList2 {
    album: Vec<SubsonicAlbum>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct SubsonicAlbum {
    id: String,
    name: String,
    artist: Option<String>,
    artist_id: Option<String>,
    created: Option<String>,
    year: Option<u64>,
    song: Option<Vec<SubsonicSong>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct SubsonicArtistDetails {
    id: String,
    name: String,
    album: Vec<SubsonicAlbum>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct SubsonicSong {
    id: String,
    title: String,
    album: Option<String>,
    artist: Option<String>,
    album_id: Option<String>,
    duration: Option<u64>,
    track: Option<u64>,
    genre: Option<String>,
    created: Option<String>,
    starred: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubsonicLyricsList {
    structured_lyrics: Option<Vec<SubsonicStructuredLyric>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubsonicStructuredLyric {
    line: Vec<SubsonicLyricLine>,
    synced: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubsonicLyricLine {
    #[serde(default)]
    start: u64,
    value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchResult3 {
    song: Option<Vec<SubsonicSong>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RandomSongs {
    song: Option<Vec<SubsonicSong>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubsonicPlaylists {
    playlist: Vec<SubsonicPlaylistEntry>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct SubsonicPlaylistEntry {
    id: String,
    name: String,
    #[serde(default)]
    song_count: u64,
    #[serde(default)]
    duration: u64,
    #[serde(default, rename = "created")]
    date_created: String,
    #[serde(default)]
    public: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct SubsonicPlaylistWithSongs {
    id: String,
    name: String,
    #[serde(default)]
    entry: Vec<SubsonicSong>,
    #[serde(default)]
    song_count: u64,
    #[serde(default)]
    duration: u64,
    #[serde(default, rename = "created")]
    date_created: String,
}

impl Client {
    pub async fn new(
        server_url: &String,
        username: &String,
        password: &String,
    ) -> Option<Arc<Self>> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("! Failed to build HTTP client");
        let device_id = random_string();

        let salt = random_string();
        let token = format!("{:x}", md5::compute(format!("{}{}", password, salt)));

        let url = format!(
            "{}/rest/ping.view?u={}&t={}&s={}&v=1.16.1&c=navidrome-tui&f=json",
            server_url, username, token, salt
        );

        let response = http_client.get(&url).send().await;

        match response {
            Ok(json) => {
                let value = match json.json::<SubsonicResponseRoot>().await {
                    Ok(v) => v,
                    Err(e) => {
                        println!(" ! Error authenticating: {:#?}", e);
                        std::process::exit(1);
                    }
                };

                if value.response.status != "ok" {
                    println!(" ! Auth Error: {:?}", value.response.error);
                    std::process::exit(1);
                }

                let sanitized_url =
                    server_url.replace("://", "_").replace(":", "_").replace("/", "_");
                let server_id = format!("{}-{}", sanitized_url, username); // Subsonic doesn't return server ID

                Some(Arc::new(Self {
                    base_url: server_url.clone(),
                    server_id,
                    http_client,
                    user_id: username.clone(),
                    user_name: username.clone(),
                    token,
                    salt,
                    device_id,
                }))
            }
            Err(e) => {
                println!(" ! Error authenticating: {:#?}", e);
                None
            }
        }
    }

    pub async fn from_cache(base_url: &str, server_id: &String, entry: &AuthEntry) -> Arc<Self> {
        Arc::new(Self {
            base_url: base_url.to_string(),
            server_id: server_id.to_string(),
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("! Failed to build HTTP client"),
            user_id: entry.username.clone(),
            user_name: entry.username.clone(),
            token: entry.token.clone(),
            salt: entry.salt.clone(),
            device_id: entry.device_id.clone(),
        })
    }

    fn auth_query(&self) -> String {
        format!(
            "u={}&t={}&s={}&v=1.16.1&c=navidrome-tui&f=json",
            self.user_name, self.token, self.salt
        )
    }

    pub async fn validate_token(&self) -> bool {
        let url = format!("{}/rest/ping.view?{}", self.base_url, self.auth_query());
        match self.http_client.get(&url).timeout(Duration::from_secs(5)).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let root: Result<SubsonicResponseRoot, _> = response.json().await;
                    if let Ok(data) = root {
                        return data.response.status == "ok";
                    }
                }
                false
            }
            Err(_) => false,
        }
    }

    pub async fn probe_server(client: &reqwest::Client, base: &str) -> (String, NetworkQuality) {
        let base = base.trim_end_matches('/');

        if let Some(ms) = Self::probe_latency(client, base).await {
            return (base.to_string(), NetworkQuality::classify(ms));
        }

        if base.starts_with("https://") {
            let fallback = base.replacen("https://", "http://", 1);

            if let Some(ms) = Self::probe_latency(client, &fallback).await {
                let confirm = Confirm::with_theme(&DialogTheme::default())
                    .with_prompt(
                        "The server is not responding over HTTPS, but HTTP works. Switch to HTTP?",
                    )
                    .default(true)
                    .wait_for_newline(true)
                    .interact_opt()
                    .unwrap_or(None);

                if confirm.unwrap_or(false) {
                    println!(" - Switched to HTTP. Consider updating your configuration file.");
                    return (fallback, NetworkQuality::classify(ms));
                } else {
                    println!(" - HTTPS failed and HTTP fallback was declined. Exiting.");
                    std::process::exit(1);
                }
            }
        }

        (base.to_string(), NetworkQuality::CzechTrain)
    }

    async fn probe_latency(client: &reqwest::Client, base: &str) -> Option<u128> {
        let url = format!(
            "{}/rest/ping.view?v=1.16.1&c=navidrome-tui&f=json",
            base.trim_end_matches('/')
        );
        let start = std::time::Instant::now();

        match client.get(&url).timeout(Duration::from_secs(10)).send().await {
            Ok(resp) if resp.status().is_success() => Some(start.elapsed().as_millis()),
            _ => None,
        }
    }

    pub async fn network_quality(client: &reqwest::Client, base: &str) -> NetworkQuality {
        match Self::probe_latency(client, base).await {
            Some(ms) => NetworkQuality::classify(ms),
            None => NetworkQuality::CzechTrain,
        }
    }

    pub async fn music_libraries(&self) -> Result<Vec<LibraryView>, reqwest::Error> {
        let url = format!("{}/rest/getMusicFolders.view?{}", self.base_url, self.auth_query());

        let req = self.http_client.get(&url);

        let root: SubsonicResponseRoot = match self.get_json_with_retry(req).await {
            Ok(v) => v,
            Err(e) => {
                log::error!("Failed to fetch library views: {}", e);
                return Ok(vec![]);
            }
        };

        let mut music_libs = vec![];
        if let Some(folders) = root.response.music_folders {
            for f in folders.music_folder {
                music_libs.push(LibraryView {
                    id: f.id.to_string(),
                    name: f.name.unwrap_or_else(|| "Music".to_string()),
                    collection_type: Some("music".to_string()),
                    selected: false,
                });
            }
        }
        Ok(music_libs)
    }

    pub async fn artists(&self, search_term: String) -> Result<Vec<Artist>, reqwest::Error> {
        let url = format!("{}/rest/getIndexes.view?{}", self.base_url, self.auth_query());
        let req = self.http_client.get(&url);

        let root: SubsonicResponseRoot = match self.get_json_with_retry(req).await {
            Ok(a) => a,
            Err(e) => {
                log::error!("Failed to fetch artists: {}", e);
                return Ok(vec![]);
            }
        };

        let mut artists = vec![];
        if let Some(indexes) = root.response.indexes {
            for index in indexes.index {
                for a in index.artist {
                    let is_match = search_term.is_empty()
                        || a.name.to_lowercase().contains(&search_term.to_lowercase());
                    if is_match {
                        artists.push(Artist {
                            id: a.id,
                            name: a.name,
                            user_data: UserData {
                                is_favorite: a.starred.is_some(),
                                ..Default::default()
                            },
                            ..Default::default()
                        });
                    }
                }
            }
        }

        Ok(artists)
    }

    pub async fn albums(&self, library_id: Option<&String>) -> Result<Vec<Album>, reqwest::Error> {
        let size = 500;
        let mut offset = 0;
        let mut all_albums = vec![];

        loop {
            let mut url = format!(
                "{}/rest/getAlbumList2.view?type=alphabeticalByArtist&size={}&offset={}&{}",
                self.base_url,
                size,
                offset,
                self.auth_query()
            );
            if let Some(lib) = library_id {
                url.push_str(&format!("&musicFolderId={}", lib));
            }

            let req = self.http_client.get(&url);
            let root: SubsonicResponseRoot = match self.get_json_with_retry(req).await {
                Ok(parsed) => parsed,
                Err(e) => {
                    log::warn!("Failed to fetch albums: {}", e);
                    break;
                }
            };

            let mut count = 0;
            if let Some(list) = root.response.album_list2 {
                for a in list.album {
                    count += 1;
                    all_albums.push(Album {
                        id: a.id.clone(),
                        name: a.name.clone(),
                        album_artists: vec![Artist {
                            id: a.artist_id.unwrap_or_default(),
                            name: a.artist.unwrap_or_default(),
                            ..Default::default()
                        }],
                        date_created: a.created.unwrap_or_default(),
                        ..Default::default()
                    });
                }
            }

            if count < size {
                break;
            }
            offset += size;
        }

        Ok(all_albums)
    }

    pub async fn album_tracks(&self, id: &str) -> Result<Vec<DiscographySong>, reqwest::Error> {
        let url = format!("{}/rest/getAlbum.view?id={}&{}", self.base_url, id, self.auth_query());
        let req = self.http_client.get(&url);

        let root: SubsonicResponseRoot = match self.get_json_with_retry(req).await {
            Ok(d) => d,
            Err(e) => {
                log::error!("Failed to fetch album_tracks for album {}: {}", id, e);
                return Ok(vec![]);
            }
        };

        let mut songs = vec![];
        if let Some(album) = root.response.album {
            if let Some(list) = album.song {
                for s in list {
                    songs.push(self.subsonic_song_to_discography_song(s));
                }
            }
        }
        Ok(songs)
    }

    pub async fn discography(&self, id: &str) -> Result<Vec<DiscographySong>, reqwest::Error> {
        let url = format!("{}/rest/getArtist.view?id={}&{}", self.base_url, id, self.auth_query());
        let req = self.http_client.get(&url);

        let root: SubsonicResponseRoot = match self.get_json_with_retry(req).await {
            Ok(d) => d,
            Err(e) => {
                log::error!("Failed to fetch artist info for {}: {}", id, e);
                return Ok(vec![]);
            }
        };

        let mut all_songs = vec![];
        if let Some(artist) = root.response.artist {
            for album in artist.album {
                if let Ok(mut tracks) = self.album_tracks(&album.id).await {
                    all_songs.append(&mut tracks);
                }
            }
        }
        Ok(all_songs)
    }

    pub async fn search_tracks(
        &self,
        search_term: String,
    ) -> Result<Vec<DiscographySong>, reqwest::Error> {
        let url = format!(
            "{}/rest/search3.view?query={}&songCount=100&{} ",
            self.base_url,
            search_term,
            self.auth_query()
        );
        let req = self.http_client.get(&url);

        let root: SubsonicResponseRoot = match self.get_json_with_retry(req).await {
            Ok(d) => d,
            Err(e) => {
                log::error!("Search tracks failed for '{}': {}", search_term, e);
                return Ok(vec![]);
            }
        };

        let mut songs = vec![];
        if let Some(res) = root.response.search_result3 {
            if let Some(list) = res.song {
                for s in list {
                    songs.push(self.subsonic_song_to_discography_song(s));
                }
            }
        }
        Ok(songs)
    }

    pub async fn random_tracks(
        &self,
        tracks_n: usize,
        _only_played: bool,
        _only_unplayed: bool,
        only_favorite: bool,
    ) -> Result<Vec<DiscographySong>, Box<dyn Error>> {
        let url = format!(
            "{}/rest/getRandomSongs.view?size={}&{}",
            self.base_url,
            tracks_n,
            self.auth_query()
        );
        // Subsonic doesn't really have "only_played" / "only_unplayed" natively in random filter,
        // we can filter manually if needed, but it's hard.
        let req = self.http_client.get(&url);

        let root: SubsonicResponseRoot = match self.get_json_with_retry(req).await {
            Ok(d) => d,
            Err(e) => {
                log::error!("Random tracks request failed: {}", e);
                return Ok(vec![]);
            }
        };

        let mut songs = vec![];
        if let Some(rs) = root.response.random_songs {
            if let Some(list) = rs.song {
                for s in list {
                    let mut keep = true;
                    if only_favorite && s.starred.is_none() {
                        keep = false;
                    }
                    if keep {
                        songs.push(self.subsonic_song_to_discography_song(s));
                    }
                }
            }
        }
        Ok(songs)
    }

    pub async fn instant_playlist(
        &self,
        track_id: &String,
        tracks_n: Option<usize>,
    ) -> Result<Vec<DiscographySong>, Box<dyn Error>> {
        let size = tracks_n.unwrap_or(50);
        let url = format!(
            "{}/rest/getSimilarSongs2.view?id={}&count={}&{}",
            self.base_url,
            track_id,
            size,
            self.auth_query()
        );
        let req = self.http_client.get(&url);

        let response = req.send().await;

        match response {
            Ok(json) => {
                if let Ok(_root) = json.json::<SubsonicResponseRoot>().await {
                    let _songs: Vec<DiscographySong> = vec![];
                    // Some servers use similarSongs2 -> song
                    // If not found, just return random
                    return self.random_tracks(size, false, false, false).await;
                }
            }
            Err(_) => {}
        }
        Ok(vec![])
    }

    pub async fn lyrics(&self, song_id: &String) -> Result<Vec<Lyric>, reqwest::Error> {
        let url = format!(
            "{}/rest/getLyricsBySongId.view?id={}&{}",
            self.base_url,
            song_id,
            self.auth_query()
        );
        let req = self.http_client.get(&url);

        let root: SubsonicResponseRoot = match self.get_json_with_retry(req).await {
            Ok(d) => d,
            Err(e) => {
                log::error!("Lyrics request failed for '{}': {}", song_id, e);
                return Ok(vec![]);
            }
        };

        let mut lyrics = vec![];
        if let Some(lyrics_list) = root.response.lyrics_list {
            if let Some(structured) = lyrics_list.structured_lyrics {
                for sl in &structured {
                    if sl.synced {
                        for line in &sl.line {
                            lyrics.push(Lyric {
                                text: line.value.clone(),
                                start: line.start * 10_000,
                            });
                        }
                    }
                }
                if lyrics.is_empty() {
                    for sl in &structured {
                        if !sl.synced {
                            for line in &sl.line {
                                lyrics.push(Lyric {
                                    text: line.value.clone(),
                                    start: 0,
                                });
                            }
                        }
                    }
                }
            }
        }
        Ok(lyrics)
    }

    pub async fn download_cover_art(
        &self,
        item_id: &String,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "{}/rest/getCoverArt.view?id={}&size=512&{}",
            self.base_url,
            item_id,
            self.auth_query()
        );
        let response = self.http_client.get(&url).send().await?;

        let content_type = match response.headers().get("Content-Type") {
            Some(c) => c.to_str()?,
            None => "",
        };
        let extension = match content_type {
            "image/png" => "png",
            "image/jpeg" => "jpeg",
            "image/jpg" => "jpg",
            "image/webp" => "webp",
            _ => "jpg",
        };

        let bytes = response.bytes().await?.to_vec();

        let cover_dir = data_dir().unwrap().join("navidrome-tui").join("covers");
        tokio::fs::create_dir_all(&cover_dir).await?;

        let final_path = cover_dir.join(format!("{}.{}", item_id, extension));
        let tmp_path = cover_dir.join(format!("{}.{}.part", item_id, extension));

        tokio::fs::write(&tmp_path, &bytes).await?;
        tokio::fs::rename(&tmp_path, &final_path).await?;

        Ok(format!("{}.{}", item_id, extension))
    }

    pub async fn playlists(&self, search_term: String) -> Result<Vec<Playlist>, reqwest::Error> {
        let url = format!("{}/rest/getPlaylists.view?{}", self.base_url, self.auth_query());
        let root: SubsonicResponseRoot =
            match self.get_json_with_retry(self.http_client.get(&url)).await {
                Ok(d) => d,
                Err(_) => return Ok(vec![]),
            };
        let mut results = vec![];
        if let Some(pl) = root.response.playlists {
            for p in pl.playlist {
                let matches = search_term.is_empty()
                    || p.name.to_lowercase().contains(&search_term.to_lowercase());
                if matches {
                    results.push(Playlist {
                        id: p.id,
                        name: p.name,
                        date_created: p.date_created,
                        run_time_ticks: p.duration * 10_000_000,
                        ..Default::default()
                    });
                }
            }
        }
        Ok(results)
    }

    pub async fn playlist(
        &self,
        id: &str,
        _limit: Option<usize>,
    ) -> Result<PlaylistResult, reqwest::Error> {
        let url =
            format!("{}/rest/getPlaylist.view?id={}&{}", self.base_url, id, self.auth_query());
        let root: SubsonicResponseRoot =
            match self.get_json_with_retry(self.http_client.get(&url)).await {
                Ok(d) => d,
                Err(_) => return Ok(PlaylistResult { items: vec![], total_record_count: 0 }),
            };
        if let Some(pl) = root.response.playlist {
            let total = pl.song_count;
            let items =
                pl.entry.into_iter().map(|s| self.subsonic_song_to_discography_song(s)).collect();
            return Ok(PlaylistResult { items, total_record_count: total });
        }
        Ok(PlaylistResult { items: vec![], total_record_count: 0 })
    }

    pub async fn stopped(
        &self,
        id: Option<String>,
        position: Option<u64>,
    ) -> Result<(), reqwest::Error> {
        // Scrobble as submission=true when track stops (finished)
        if let Some(track_id) = id {
            let time_ms = position.unwrap_or(0) / 10_000;
            let url = format!(
                "{}/rest/scrobble.view?id={}&time={}&submission=true&{}",
                self.base_url,
                track_id,
                time_ms,
                self.auth_query()
            );
            let _ = self.http_client.get(&url).send().await;
        }
        Ok(())
    }
    pub async fn playing(&self, id: &str) -> Result<(), reqwest::Error> {
        // Scrobble submission=false when starting playback ("Now playing")
        let url = format!(
            "{}/rest/scrobble.view?id={}&submission=false&{}",
            self.base_url,
            id,
            self.auth_query()
        );
        let _ = self.http_client.get(&url).send().await;
        Ok(())
    }
    pub async fn move_playlist_item(
        &self,
        _item_id: &str,
        _playlist_id: &str,
        _new_index: usize,
    ) -> Result<(), reqwest::Error> {
        Ok(())
    }
    pub async fn add_to_playlist(
        &self,
        track_id: &str,
        playlist_id: &str,
    ) -> Result<(), reqwest::Error> {
        let url = format!(
            "{}/rest/updatePlaylist.view?playlistId={}&songIdToAdd={}&{}",
            self.base_url,
            playlist_id,
            track_id,
            self.auth_query()
        );
        let _ = self.http_client.get(&url).send().await;
        Ok(())
    }
    pub async fn remove_from_playlist(
        &self,
        track_id: &str,
        playlist_id: &str,
    ) -> Result<(), reqwest::Error> {
        let url = format!(
            "{}/rest/updatePlaylist.view?playlistId={}&songIndexToRemove={}&{}",
            self.base_url,
            playlist_id,
            track_id,
            self.auth_query()
        );
        let _ = self.http_client.get(&url).send().await;
        Ok(())
    }
    pub async fn update_playlist(&self, playlist: &Playlist) -> Result<(), reqwest::Error> {
        let url = format!(
            "{}/rest/updatePlaylist.view?playlistId={}&name={}&{}",
            self.base_url,
            playlist.id,
            playlist.name,
            self.auth_query()
        );
        let _ = self.http_client.get(&url).send().await;
        Ok(())
    }
    pub async fn delete_playlist(&self, id: &str) -> Result<(), reqwest::Error> {
        let url =
            format!("{}/rest/deletePlaylist.view?id={}&{}", self.base_url, id, self.auth_query());
        let _ = self.http_client.get(&url).send().await;
        Ok(())
    }
    pub async fn create_playlist(
        &self,
        name: &str,
        _public: bool,
    ) -> Result<String, reqwest::Error> {
        let url = format!(
            "{}/rest/createPlaylist.view?name={}&{}",
            self.base_url,
            name,
            self.auth_query()
        );
        // Parse the response to get the new playlist id
        if let Ok(root) =
            self.get_json_with_retry::<SubsonicResponseRoot>(self.http_client.get(&url)).await
        {
            if let Some(pl) = root.response.playlist {
                return Ok(pl.id);
            }
        }
        Ok(String::new())
    }
    pub fn song_url_sync(&self, song_id: &String, _transcoding: Option<&Transcoding>) -> String {
        format!("{}/rest/stream.view?id={}&{}", self.base_url, song_id, self.auth_query())
    }

    pub async fn report_progress(&self, pr: &ProgressReport) -> Result<(), reqwest::Error> {
        let time = pr.position_ticks / 10_000_000;
        // if paused, just report time, if finished, scrobble (submission=true)
        // Since we don't have perfect playback state, we assume if it's over 30s we can scrobble or
        // just submission=false for progress
        let submission = "false";
        let url = format!(
            "{}/rest/scrobble.view?id={}&time={}&submission={}&{}",
            self.base_url,
            pr.item_id,
            time,
            submission,
            self.auth_query()
        );
        let client = reqwest::Client::new();
        let _ = client.post(&url).send().await;
        Ok(())
    }

    pub async fn set_favorite(
        &self,
        item_id: &String,
        favorite: bool,
    ) -> Result<(), reqwest::Error> {
        // Use Subsonic star/unstar endpoints (correct way) instead of setRating
        let action = if favorite { "star" } else { "unstar" };
        let url =
            format!("{}/rest/{}.view?id={}&{}", self.base_url, action, item_id, self.auth_query());
        let _ = self.http_client.get(&url).send().await;
        Ok(())
    }

    async fn get_json_with_retry<T: serde::de::DeserializeOwned>(
        &self,
        req: reqwest::RequestBuilder,
    ) -> Result<T, reqwest::Error> {
        const MAX_RETRIES: usize = 3;
        const RETRY_DELAY_MS: u64 = 500;

        let mut attempt = 0;

        loop {
            let cloned = match req.try_clone() {
                Some(c) => c,
                None => return req.send().await?.json::<T>().await,
            };

            match cloned.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        return resp.json::<T>().await;
                    }
                    if status.is_client_error() {
                        return resp.json::<T>().await;
                    }
                    attempt += 1;
                }
                Err(_) => {
                    attempt += 1;
                }
            }

            if attempt >= MAX_RETRIES {
                let final_req = match req.try_clone() {
                    Some(r) => r,
                    None => return req.send().await?.json::<T>().await,
                };
                return final_req.send().await?.json::<T>().await;
            }

            tokio::time::sleep(std::time::Duration::from_millis(attempt as u64 * RETRY_DELAY_MS))
                .await;
        }
    }

    fn subsonic_song_to_discography_song(&self, s: SubsonicSong) -> DiscographySong {
        let artist_name = s.artist.clone().unwrap_or_default();
        let album_name = s.album.clone().unwrap_or_default();
        let duration_ms = s.duration.unwrap_or(0) * 1000;
        let ticks = duration_ms * 10000;
        let mut d_song = DiscographySong::default();
        d_song.id = s.id.clone();
        d_song.name = s.title.clone();
        d_song.album = album_name;
        d_song.album_artist = artist_name.clone();
        d_song.album_id = s.album_id.unwrap_or_default();
        d_song.run_time_ticks = ticks;
        d_song.user_data.is_favorite = s.starred.is_some();
        d_song.artists = vec![artist_name.clone()];
        d_song.album_artists =
            vec![Artist { id: String::new(), name: artist_name.clone(), ..Default::default() }];
        d_song.genres = s.genre.into_iter().collect();
        d_song
    }
}

pub fn random_string() -> String {
    let charset = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    random_string::generate(10, charset)
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Artist {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub run_time_ticks: u64,
    #[serde(default)]
    pub user_data: UserData,
    #[serde(default)]
    pub date_created: String,
}

impl Searchable for Artist {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct UserData {
    #[serde(default)]
    pub is_favorite: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DiscographySongUserData {
    #[serde(default)]
    pub play_count: u64,
    #[serde(default)]
    pub is_favorite: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DiscographySong {
    pub album: String,
    pub album_artist: String,
    pub album_artists: Vec<Artist>,
    pub album_id: String,
    pub artists: Vec<String>,
    pub date_created: String,
    pub genres: Vec<String>,
    pub has_lyrics: bool,
    pub id: String,
    pub name: String,
    pub playlist_item_id: String,
    pub index_number: u64,
    pub parent_id: String,
    pub parent_index_number: u64,
    pub premiere_date: String,
    pub production_year: u64,
    pub run_time_ticks: u64,
    pub server_id: String,
    pub user_data: DiscographySongUserData,
    pub download_status: DownloadStatus,
    pub disliked: bool,
}

impl Searchable for DiscographySong {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
}

impl<'r> FromRow<'r, sqlx::sqlite::SqliteRow> for DiscographySong {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.get("id"),
            album: row.get("album"),
            album_artist: row.get("album_artist"),
            album_id: row.get("album_id"),
            date_created: row.get("date_created"),
            name: row.get("name"),
            parent_id: row.get("parent_id"),
            premiere_date: row.get("premiere_date"),
            server_id: row.get("server_id"),
            album_artists: serde_json::from_str(row.get::<&str, _>("album_artists"))
                .unwrap_or_default(),
            artists: serde_json::from_str(row.get::<&str, _>("artists")).unwrap_or_default(),
            genres: serde_json::from_str(row.get::<&str, _>("genres")).unwrap_or_default(),
            user_data: serde_json::from_str(row.get::<&str, _>("user_data"))
                .unwrap_or_else(|_| DiscographySongUserData::default()),
            has_lyrics: row.get::<i32, _>("has_lyrics") != 0,
            index_number: row.get("index_number"),
            parent_index_number: row.get("parent_index_number"),
            production_year: row.get("production_year"),
            run_time_ticks: row.get("run_time_ticks"),
            playlist_item_id: row.get("playlist_item_id"),
            download_status: serde_json::from_str(row.get::<&str, _>("download_status"))
                .unwrap_or(DownloadStatus::NotDownloaded),
            disliked: row.get::<i32, _>("disliked") != 0,
            ..Default::default()
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LibraryView {
    pub id: String,
    pub name: String,
    pub collection_type: Option<String>,
    #[serde(skip)]
    pub selected: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Lyric {
    pub text: String,
    pub start: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProgressReport {
    pub volume_level: u64,
    pub is_paused: bool,
    pub position_ticks: u64,
    pub playback_start_time_ticks: u64,
    pub media_source_id: String,
    pub can_seek: bool,
    pub item_id: String,
    pub event_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Album {
    pub name: String,
    pub id: String,
    pub album_artists: Vec<Artist>,
    pub user_data: UserData,
    pub date_created: String,
    pub parent_id: String,
    pub run_time_ticks: u64,
    pub premiere_date: String,
}

impl Searchable for Album {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Playlist {
    pub name: String,
    pub server_id: String,
    pub id: String,
    pub date_created: String,
    pub run_time_ticks: u64,
    pub user_data: UserData,
    pub child_count: u64,
    pub parent_id: String,
}

impl Searchable for Playlist {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
}
