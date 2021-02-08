use yew::prelude::*;

use crate::binder;


const VIDEO_PLAYER_ID: &str = "video-player";
const VIDEO_CONTAINER_ID: &str = "video-container";


pub struct Video {
    pub src: String,
    controls: bool,

    pub playing: bool,
    pub muted: bool,
    pub volume: u32,
    pub pct_pos: f32,

    is_fullscreen: bool,
}

impl  Video {
    pub fn new(controls: bool, src: String) -> Self {
        Self {
            src,
            controls,

            playing: false,
            muted: false,
            volume: 100,
            pct_pos: 0f32,

            is_fullscreen: false,
        }
    }

    pub fn play(&mut self) {
        if !self.playing {
            self.playing = true;
            binder::play_video(&VIDEO_PLAYER_ID);
        }
    }

    pub fn pause(&mut self) {
        if self.playing {
            self.playing = false;
            binder::pause_video(&VIDEO_PLAYER_ID);
        }
    }

    pub fn mute(&mut self) {
        if !self.muted {
            self.muted = true;
            binder::mute_video(&VIDEO_PLAYER_ID);
        }
    }

    pub fn unmute(&mut self) {
        if self.muted {
            self.muted = false;
            binder::unmute_video(&VIDEO_PLAYER_ID);
        }
    }

    pub fn set_vol(&mut self, vol: u32) {
        self.volume = vol;

        let modified = self.volume as f32 / 100f32;
        binder::set_vol(&VIDEO_PLAYER_ID, modified);
    }

    pub fn seek(&mut self, pos: u32) {
        binder::seek_video(&VIDEO_PLAYER_ID, pos);
    }

    pub fn toggle_fullscreen(&mut self) {
        if self.is_fullscreen {
            binder::minimise(&VIDEO_CONTAINER_ID);
            self.is_fullscreen = false;
        } else {
            binder::maximise(&VIDEO_CONTAINER_ID);
            self.is_fullscreen = true;
        }
    }

    pub fn get_duration(&mut self) -> u32 {
        return binder::get_duration(&VIDEO_PLAYER_ID)
    }

    pub fn get_pos(&mut self) -> u32 {
        return binder::get_pos(&VIDEO_PLAYER_ID)
    }

    pub fn update_pos(&mut self) {
        // this is fine, i hope.
        self.pct_pos = binder::get_pct_done(&VIDEO_PLAYER_ID);

        if self.pct_pos >= 100f32 {
            self.playing = false;
        }
    }

    pub fn view(&self) -> Html {

        html! {
            <video id="video-player" class="rounded-lg w-full h-full" controls=self.controls>
                <source src=&self.src/>
            </video>
        }
    }
}
