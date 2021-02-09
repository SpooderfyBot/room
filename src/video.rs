use wasm_bindgen::prelude::*;
use yew::prelude::*;

use serde::{Serialize, Deserialize};

use std::collections::VecDeque;
use std::borrow::Borrow;

use crate::binder;
use crate::webtorrent;
use yew::services::ConsoleService;
use wasm_bindgen::__rt::core::sync::atomic::AtomicBool;
use wasm_bindgen::__rt::core::sync::atomic::Ordering::Relaxed;

const VIDEO_PLAYER_ID: &str = "video-player";
const VIDEO_CONTAINER_ID: &str = "video-container";


/// A video track that can be loaded by the video player, this should contain
/// all relevant data needed for the video player to select the correct
/// settings and display the extra info.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueuedVideo {
    /// The video title
    title: String,

    /// The video url
    url: String,
}


/// The type of video to be played.
///
/// Currently only direct links and torrents can be played but plans to
/// add more support soon:tm:
#[derive(Clone, Debug, PartialEq)]
pub enum VideoType {
    /// A direct video url.
    Direct(String),

    /// A torrent link extracted from the metadata.
    Torrent(JsValue),
}


/// A extracted video track, containing a reference to it's parent queued track.
pub struct VideoTrack {
    /// The name of the video, this is the parent's name if using a
    /// Direct link.
    pub video_name: String,

    /// The type of video e.g. Torrent(torrentID)
    pub video_type: VideoType,

    /// The parent index.
    pub playlist_id: usize,
}


/// The media playlist manager.
pub struct MediaPlaylist {
    /// The videos that are set by the user, these can contain torrents and
    /// playlists so these are in-need of extraction.
    queued_videos: Vec<QueuedVideo>,

    /// The videos that have been extracted from any torrents or playlists.
    extracted_videos: VecDeque<VideoTrack>,

    callback: Callback<(usize, Vec<JsValue>)>,
}

impl MediaPlaylist {
    pub fn new(cb: Callback<(usize, Vec<JsValue>)>) -> Self {
        Self {
            queued_videos: Vec::new(),
            extracted_videos: VecDeque::new(),
            callback: cb,
        }
    }

    pub fn rotate_next(&mut self) {
        self.extracted_videos.rotate_left(1);
    }

    pub fn rotate_prev(&mut self) {
        self.extracted_videos.rotate_left(1);
    }

    pub fn append_video(&mut self, video: QueuedVideo) {
        let index = self.queued_videos.len();
        self.extract_video(index, &video);
        self.queued_videos.push(video);
    }

    pub fn delete_current(&mut self) {
        let maybe_video = self.extracted_videos.remove(0);
        let video = if let Some(video) = maybe_video {
            video
        } else {
            return
        };

        let mut remove_queued = false;
        for other in self.extracted_videos.iter() {
            if video.playlist_id == other.playlist_id {
                remove_queued = true;
                break;
            }
        }

        if remove_queued | (self.extracted_videos.len() == 0) {
            self.queued_videos.remove(video.playlist_id);
        }
    }

    /// Gets the currently selected track according the to the queue of
    /// extracted videos.
    pub fn get_video(&self) -> (String, VideoType) {
        let selected = self.extracted_videos[0].borrow();
        let title = selected.video_name.clone();
        let video_type = selected.video_type.clone();

        (title, video_type)
    }

    /// Clones the queued videos and returns them, useful for syncing.
    pub fn get_queued_videos(&self) -> Vec<QueuedVideo> {
        self.queued_videos.clone()
    }

    /// Sets the queued videos to the given vector, this also clears
    /// and re-extracts the queue therefore synchronising the extracted
    /// queue as well.
    pub fn set_queued_videos(&mut self, videos: Vec<QueuedVideo>) {
        self.queued_videos = videos;
        self.extract_videos();
    }

    fn extract_videos(&mut self) {
        self.extracted_videos.clear();

        let dup = self.queued_videos.clone();
        let iterator = dup.iter().enumerate();

        for (index, queued) in iterator {
            self.extract_video(index, queued);
        }
    }

    fn extract_video(&mut self, index: usize, video: &QueuedVideo) {
        let video_name = video.title.clone();
        let url = video.url.clone();

        if url.starts_with("magnet") | url.ends_with(".torrent") {
            let temp = self.callback.clone();
            let cb = Closure::once_into_js(
                move |files: Vec<JsValue>| temp.emit((index, files))
            );

            webtorrent::binder::extract_files(
                &url,
                cb,
            );
        } else {
            let track = VideoTrack {
                video_name,
                video_type: VideoType::Direct(url),
                playlist_id: index,
            };
            self.extracted_videos.push_back(track);
        };
    }

    pub fn submit_video(&mut self, index: usize, videos: Vec<JsValue>) {
        let parent_name = match self.queued_videos.get(index) {
            Some(p) => p.title.clone(),
            None => return,
        };

        for video in videos {
            let part = webtorrent::binder::extract_title(&video);
            let title = format!("{} - {}", &parent_name, part);

            ConsoleService::log(&title);

            let track = VideoTrack {
                video_name: title,
                video_type: VideoType::Torrent(video),
                playlist_id: index,
            };
            self.extracted_videos.push_back(track);
        }

    }

    /// The length of the DeQueue of extracted videos, this takes into account
    /// tracks contained in playlists etc...
    #[inline]
    pub fn len(&self) -> usize {
        self.extracted_videos.len()
    }

    /// The length of queued videos contained in the vector.
    #[inline]
    pub fn queue_len(&self) -> usize {
        self.queued_videos.len()
    }
}


/// A video struct that wraps the <video> element tag.
pub struct Video {
    /// The selected video to play.
    pub video: VideoType,

    /// Hide or show default controls.
    controls: bool,

    /// Signals if the video is playing or not.
    pub playing: bool,

    /// Signals if the video is muted or not.
    pub muted: bool,

    /// The volume of the video from 0 -> 100
    pub volume: u32,

    /// The percentage position of the video.
    pub pct_pos: f32,

    /// Signals if the player is in a full screen or not.
    is_fullscreen: bool,

    torrent_rendered: AtomicBool,

    pub ready_state: u32,
}

impl  Video {
    /// Creates a new video element with a set video and controls.
    pub fn new(controls: bool, video: VideoType) -> Self {
        Self {
            video,
            controls,

            playing: false,
            muted: false,
            volume: 100,
            pct_pos: 0f32,

            is_fullscreen: false,
            torrent_rendered: AtomicBool::new(false),
            ready_state: 1,
        }
    }

    /// Starts the video playing if it isn't already,
    /// this invokes the JS function.
    pub fn play(&mut self) {
        if !self.playing {
            self.playing = true;
            binder::play_video(&VIDEO_PLAYER_ID);
        }
    }

    /// Pauses the video if it is already playing, this wraps the JS function.
    pub fn pause(&mut self) {
        if self.playing {
            self.playing = false;
            binder::pause_video(&VIDEO_PLAYER_ID);
        }
    }

    /// Sets the video to a passed video, this also pauses the video, resets
    /// the position and reloads the video of the player.
    pub fn set_video(&mut self, video: VideoType) {
        self.torrent_rendered.store(false, Relaxed);
        self.video = video;
        self.pause();
        self.seek(0);
        binder::reload_video(&VIDEO_PLAYER_ID)
    }

    /// Mutes the video.
    pub fn mute(&mut self) {
        if !self.muted {
            self.muted = true;
            binder::mute_video(&VIDEO_PLAYER_ID);
        }
    }

    /// UnMutes the video.
    pub fn unmute(&mut self) {
        if self.muted {
            self.muted = false;
            binder::unmute_video(&VIDEO_PLAYER_ID);
        }
    }

    /// Sets the volume of the player from 0 -> 100
    pub fn set_vol(&mut self, vol: u32) {
        self.volume = vol;

        let modified = self.volume as f32 / 100f32;
        binder::set_vol(&VIDEO_PLAYER_ID, modified);
    }

    /// Seeks the player to a time in seconds.
    pub fn seek(&mut self, pos: u32) {
        binder::seek_video(&VIDEO_PLAYER_ID, pos);
    }

    /// Toggles the fullscreen of the player, if the player is in fullscreen
    /// it is minimised otherwise it is maximised.
    pub fn toggle_fullscreen(&mut self) {
        if self.is_fullscreen {
            binder::minimise(&VIDEO_CONTAINER_ID);
            self.is_fullscreen = false;
        } else {
            binder::maximise(&VIDEO_CONTAINER_ID);
            self.is_fullscreen = true;
        }
    }

    /// Gets the current duration of the video in seconds.
    pub fn get_duration(&mut self) -> u32 {
        return binder::get_duration(&VIDEO_PLAYER_ID)
    }

    /// Gets the current position of the player in seconds.
    pub fn get_pos(&mut self) -> u32 {
        return binder::get_pos(&VIDEO_PLAYER_ID)
    }

    /// A tick callback to update the position in the wrapper.
    pub fn update_pos(&mut self) {
        self.ready_state = binder::get_state(&VIDEO_PLAYER_ID);

        if !self.playing {
            return
        }

        // this is fine, i hope.
        self.pct_pos = binder::get_pct_done(&VIDEO_PLAYER_ID);

        if self.pct_pos >= 100f32 {
            self.playing = false;
        }
    }

    /// Renders the video to a html struct.
    pub fn view(&self) -> Html {
        let rendered = if let VideoType::Direct(url) = &self.video {
            html! {
                <video id="video-player" class="rounded-lg h-full mx-auto" style="min-width: 80%;" controls=self.controls>
                    <source src=url/>
                </video>
            }
        } else if let VideoType::Torrent(file) = &self.video {
            if !self.torrent_rendered.load(Relaxed) {
                webtorrent::binder::render_to_video(
                    &VIDEO_PLAYER_ID,
                    file,
                    500 // 500 ms
                );

                self.torrent_rendered.store(true, Relaxed);
            };

            html! {
                <video id="video-player" class="rounded-lg h-full mx-auto" style="min-width: 80%;" controls=self.controls>
                </video>
            }
        } else {
            ConsoleService::error("Failed to support video type.");
            panic!()
        };

        html! { { rendered } }
    }
}


/*
#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_append() {
        let mut playlist = MediaPlaylist::new();

        assert_eq!(playlist.queue_len(), 0);
        assert_eq!(playlist.len(), 0);

        let video = QueuedVideo {
            title: "test".to_string(),
            url: "test".to_string()
        };

        playlist.append_video(video);

        assert_eq!(playlist.queue_len(), 1);
        assert_eq!(playlist.len(), 1);
    }

    #[test]
    fn test_remove_empty() {
        let mut playlist = MediaPlaylist::new();

        assert_eq!(playlist.queue_len(), 0);
        assert_eq!(playlist.len(), 0);

        playlist.delete_current();

        assert_eq!(playlist.queue_len(), 0);
        assert_eq!(playlist.len(), 0);
    }

    #[test]
    fn test_remove_item() {
        let mut playlist = MediaPlaylist::new();

        assert_eq!(playlist.queue_len(), 0);
        assert_eq!(playlist.len(), 0);

        playlist.delete_current();

        let video = QueuedVideo {
            title: "test".to_string(),
            url: "test".to_string()
        };

        playlist.append_video(video);

        assert_eq!(playlist.queue_len(), 1);
        assert_eq!(playlist.len(), 1);

        playlist.delete_current();

        assert_eq!(playlist.queue_len(), 0);
        assert_eq!(playlist.len(), 0);
    }

    #[test]
    fn test_rotate_next_1() {
        let mut playlist = MediaPlaylist::new();

        assert_eq!(playlist.queue_len(), 0);
        assert_eq!(playlist.len(), 0);

        playlist.delete_current();

        let video = QueuedVideo {
            title: "test".to_string(),
            url: "test".to_string()
        };

        playlist.append_video(video.clone());

        assert_eq!(playlist.queue_len(), 1);
        assert_eq!(playlist.len(), 1);

        let expected_type = VideoType::Direct(video.title);

        playlist.rotate_next();
        let (_title, vid_type) = playlist.get_video();
        assert_eq!(&vid_type, &expected_type);

        playlist.rotate_next();
        let (_title, vid_type) = playlist.get_video();
        assert_eq!(&vid_type, &expected_type);
    }

    #[test]
    fn test_rotate_next_2() {
        let mut playlist = MediaPlaylist::new();

        assert_eq!(playlist.queue_len(), 0);
        assert_eq!(playlist.len(), 0);

        playlist.delete_current();

        let video = QueuedVideo {
            title: "test".to_string(),
            url: "test".to_string()
        };

        playlist.append_video(video.clone());

        let video2 = QueuedVideo {
            title: "test 2".to_string(),
            url: "test2".to_string()
        };

        playlist.append_video(video2.clone());

        assert_eq!(playlist.queue_len(), 2);
        assert_eq!(playlist.len(), 2);

        let expected_type1 = VideoType::Direct(video2.url);
        let expected_type2 = VideoType::Direct(video.url);


        playlist.rotate_next();
        let (_title, vid_type) = playlist.get_video();
        assert_eq!(&vid_type, &expected_type1);


        playlist.rotate_next();
        let (_title, vid_type) = playlist.get_video();
        assert_eq!(&vid_type, &expected_type2);
    }

    #[test]
    fn test_rotate_prev_1() {
        let mut playlist = MediaPlaylist::new();

        assert_eq!(playlist.queue_len(), 0);
        assert_eq!(playlist.len(), 0);

        playlist.delete_current();

        let video = QueuedVideo {
            title: "test".to_string(),
            url: "test".to_string()
        };

        playlist.append_video(video.clone());

        assert_eq!(playlist.queue_len(), 1);
        assert_eq!(playlist.len(), 1);

        let expected_type = VideoType::Direct(video.title);

        playlist.rotate_prev();

        let (_title, vid_type) = playlist.get_video();
        assert_eq!(&vid_type, &expected_type);

        playlist.rotate_prev();
        let (_title, vid_type) = playlist.get_video();
        assert_eq!(&vid_type, &expected_type);
    }

    #[test]
    fn test_rotate_prev_2() {
        let mut playlist = MediaPlaylist::new();

        assert_eq!(playlist.queue_len(), 0);
        assert_eq!(playlist.len(), 0);

        playlist.delete_current();

        let video = QueuedVideo {
            title: "test".to_string(),
            url: "test".to_string()
        };

        playlist.append_video(video.clone());

        let video2 = QueuedVideo {
            title: "test 2".to_string(),
            url: "test2".to_string()
        };

        playlist.append_video(video2.clone());

        assert_eq!(playlist.queue_len(), 2);
        assert_eq!(playlist.len(), 2);

        let expected_type1 = VideoType::Direct(video2.url);
        let expected_type2 = VideoType::Direct(video.url);


        playlist.rotate_prev();
        let (_title, vid_type) = playlist.get_video();
        assert_eq!(&vid_type, &expected_type1);


        playlist.rotate_prev();
        let (_title, vid_type) = playlist.get_video();
        assert_eq!(&vid_type, &expected_type2);
    }
}
*/