use yew::prelude::*;
use yew::services::{IntervalService, ConsoleService};
use yew::services::interval::IntervalTask;

use std::time::Duration;
use std::collections::VecDeque;
use std::borrow::Borrow;

use serde::{Serialize, Deserialize};

use crate::video;
use crate::opcodes;
use crate::settings;
use crate::utils::{emit_event, start_future, send_post};
use crate::websocket::{WsHandler, WebsocketMessage, WrappingWsMessage};



/// A video track that can be loaded by the video player, this should contain
/// all relevant data needed for the video player to select the correct
/// settings and display the extra info.
#[derive(Debug, Serialize, Deserialize)]
pub struct Video {
    /// The video title
    title: String,

    /// The video url
    url: String,
}


/// The payload of the `OP_SET_BULK_TRACKS` operation.
#[derive(Debug, Serialize, Deserialize)]
pub struct BulkVideos {
    /// A list of videos that can be any length >= 0
    videos: Vec<Video>,
}


/// The set component properties that can be set by the parent component.
#[derive(Properties, Clone)]
pub struct MediaPlayerProperties {
    /// The WS handle for subscribing to events.
    pub ws: WsHandler,

    /// The room id of the given room.
    pub room_id: String,
}

/// A given event that can be sent to the MediaPlayer, this can be invoked
/// by *either* the websocket or the user.
pub enum MediaPlayerEvent {
    /// Signals if the Tracks should be rotated by 1, if the bool wrapped is
    /// `true` then this event will also be emitted to the websocket otherwise
    /// it will be a localised change only, this basically is just so the
    /// websocket doesnt recursively invoke itself again.
    Next(bool),

    /// Signals if the Tracks should be rotated by -1, if the bool wrapped is
    /// `true` then this event will also be emitted to the websocket otherwise
    /// it will be a localised change only, this basically is just so the
    /// websocket doesnt recursively invoke itself again.
    Previous(bool),

    /// Adds a video contained within the `WebsocketMessage` to the end of
    /// the queue of tracks. Panics if the message is empty.
    AddVideo(WebsocketMessage),

    /// Removes the currently active video.
    RemoveVideo,

    /// An event to signal the player to get all videos it has in the queue
    /// and send them to the other clients.
    SyncTracks,

    /// An event to set the tracks of the given message originating from the
    /// SyncTracks event. The clients will then pick their
    /// queue off if the sent queue is larger or smaller. They will always
    /// pick the largest queue over a comparison of two.
    SetBulkTracks(WebsocketMessage),
}

/// The video player and details component.
///
/// This displays the help page of the player if no videos are added or set
/// otherwise it shows the video of the currently selected track according
/// to what all the other players are set to.
///
/// This components uses the VideoPlayer component to extend its base and
/// handle the actual video events itself, this just displays the title
/// and gives controls for track selection.
pub struct MediaPlayer {
    link: ComponentLink<Self>,
    videos: VecDeque<Video>,
    ws: WsHandler,
    room_id: String,
}

impl Component for MediaPlayer {
    type Message = MediaPlayerEvent;
    type Properties = MediaPlayerProperties;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let next_cb = link.callback(|_event: WebsocketMessage| MediaPlayerEvent::Next(false));
        let prev_cb = link.callback(|_event: WebsocketMessage| MediaPlayerEvent::Previous(false));
        let add_video_cb = link.callback(|event| MediaPlayerEvent::AddVideo(event));
        let remove_video_cb = link.callback(|_event: WebsocketMessage| MediaPlayerEvent::RemoveVideo);
        let sync_tracks_cb = link.callback(|_event: WebsocketMessage| MediaPlayerEvent::SyncTracks);
        let bulk_tracks_cb = link.callback(|event: WebsocketMessage| MediaPlayerEvent::SetBulkTracks(event));

        let ws = props.ws;
        ws.subscribe_to_message(settings::PLAYER_ID, opcodes::OP_NEXT, next_cb);
        ws.subscribe_to_message(settings::PLAYER_ID, opcodes::OP_PREV, prev_cb);
        ws.subscribe_to_message(settings::PLAYER_ID, opcodes::OP_ADD_TRACK, add_video_cb);
        ws.subscribe_to_message(settings::PLAYER_ID, opcodes::OP_REMOVE_TRACK, remove_video_cb);
        ws.subscribe_to_message(settings::PLAYER_ID, opcodes::OP_SYNC_TRACKS, sync_tracks_cb);
        ws.subscribe_to_message(settings::PLAYER_ID, opcodes::OP_SET_BULK_TRACKS, bulk_tracks_cb);

        let msg = WrappingWsMessage {
            opcode: opcodes::OP_SYNC_TRACKS,
            payload: None
        };
        start_future(emit_event(
            props.room_id.clone(),
            msg,
        ));

        Self {
            link,
            videos: VecDeque::new(),
            ws,
            room_id: props.room_id,
        }
    }

    /// Handles the media player events based off the Websocket and localised
    /// events.
    ///
    /// `MediaPlayerEvent::Next` and `MediaPlayerEvent::Previous` both contain
    /// a bool to signal if they should emit events to the gateway or not
    /// this is because both the user callbacks and websocket callbacks are
    /// the same just with a different bool signal, this is to cut down the
    /// size of the code base and keep it simple as unlike the video player
    /// these are not massively specialised.
    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            MediaPlayerEvent::Next(emit) => {
                if emit {
                    let msg = WrappingWsMessage {
                        opcode: opcodes::OP_NEXT,
                        payload: None
                    };

                    start_future(emit_event(self.room_id.clone(), msg))
                } else {
                    self.videos.rotate_left(1)
                }
            },
            MediaPlayerEvent::Previous(emit) => {
                if emit {
                    let msg = WrappingWsMessage {
                        opcode: opcodes::OP_PREV,
                        payload: None
                    };

                    start_future(emit_event(self.room_id.clone(), msg))
                } else {
                    self.videos.rotate_right(1)
                }
            },
            MediaPlayerEvent::AddVideo(msg) => {
                let video: Video = msg.unwrap_and_into().unwrap();
                self.videos.push_back(video);
            },
            MediaPlayerEvent::RemoveVideo => {
                self.videos.remove(0);
            },
            MediaPlayerEvent::SyncTracks => {
                let mut to_dump = BulkVideos { videos: Vec::new() };
                let existing = self.videos.drain(..);
                for video in existing {
                    to_dump.videos.push(video);
                }

                let res = serde_json::to_value(to_dump);
                let dumped = match res {
                    Ok(r) => r,
                    Err(e) => {
                        let msg = format!("{:?}", e);
                        ConsoleService::log(&msg);
                        return true;
                    }
                };

                let msg = WrappingWsMessage {
                    opcode: opcodes::OP_SET_BULK_TRACKS,
                    payload: Some(dumped)
                };

                start_future(emit_event(self.room_id.clone(), msg));

                return false;
            },
            MediaPlayerEvent::SetBulkTracks(msg) => {
                let bulk: BulkVideos = msg.unwrap_and_into().unwrap();

                if self.videos.len() > bulk.videos.len() {
                    return false;
                }

                self.videos.clear();
                for video in bulk.videos {
                    self.videos.push_back(video);
                }
            },
        }

        true
    }

    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        false
    }

    /// Renders the whole media player half of the page.
    ///
    /// This displays the help page of the player if no videos are added or set
    /// otherwise it shows the video of the currently selected track according
    /// to what all the other players are set to.
    ///
    /// This components uses the VideoPlayer component to extend its base and
    /// handle the actual video events itself, this just displays the title
    /// and gives controls for track selection.
    fn view(&self) -> Html {
        let render = if self.videos.len() > 0 {
            let (url, title) = {
                let video = self.videos[0].borrow();
                let url = video.url.clone();
                let title = video.title.clone();

                (url, title)
            };

            let next_cb = self.link.callback(|_| MediaPlayerEvent::Next(true));
            let prev_cb = self.link.callback(|_| MediaPlayerEvent::Previous(true));

            html! {
                <>
                <div class="flex justify-between items-center w-full">
                    <button onclick=next_cb class="text-white hover:text-blue-600 cursor-pointer transition duration-200 h-8 w-8">
                        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 15l-3-3m0 0l3-3m-3 3h8M3 12a9 9 0 1118 0 9 9 0 01-18 0z" />
                        </svg>
                    </button>
                    <h1 class="text-white text-3xl font-semibold w-auto">{title}</h1>
                    <button onclick=prev_cb class="text-white hover:text-blue-600 cursor-pointer transition duration-200 h-8 w-8">
                        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 9l3 3m0 0l-3 3m3-3H8m13 0a9 9 0 11-18 0 9 9 0 0118 0z" />
                        </svg>
                    </button>
                </div>
                <div class="flex justify-center w-full">
                    <div class="bg-white rounded-full w-full pt-1 px-4 my-2"></div>
                </div>
                <VideoPlayer src=url ws=self.ws.clone() room_id=self.room_id.clone()/>
                </>
            }
        } else {
            html! {
                <div class="flex flex-col w-full" style="height: 600px;">
                    <h1 class="text-white font-bold text-4xl py-4">
                        {"🎉 Your room has been made!"}
                    </h1>

                    // Part 1
                    <h1 class="text-white font-bold text-2xl py-4">
                        {"1) Add a video with Spooderfy to get started:"}
                    </h1>
                    <div class="bg-discord rounded-lg shadow-md ml-12 px-4 mb-8 w-2/3">
                        <div class="flex py-4">
                            <img
                                class="inline-block rounded-full h-12 w-12"
                                src={"https://cdn.discordapp.com/avatars/290923752475066368/4921a5665c5320be55559d1a026fca68.webp?size=128"}
                                alt=""
                            />
                            <div class="inline-block px-3 w-5/6">
                                <h1 class="text-blue-400 font-semibold">{"ハーリさん (CF8)"}</h1>
                                <p class="text-white">{"sp!addtrack https://myvideos.com/videotime \"My Title\""}</p>
                            </div>
                        </div>
                        <div class="flex py-4">
                            <img
                                class="inline-block rounded-full h-12 w-12"
                                src={"https://cdn.discordapp.com/avatars/585225058683977750/73628acbb1304b05c718f22a380767bd.png?size=128"}
                                alt=""
                            />
                            <div class="inline-block px-3 w-5/6">
                                <h1 class="text-blue-400 font-semibold">{"Spooderfy"}</h1>
                                <div class="flex items-center">
                                    <img
                                        class="inline-block rounded-full h-5 w-5"
                                        src={"https://spooderfy.com/static/images/spooderfy_white_fill.png"}
                                        alt=""
                                    />
                                    <p class="text-white font-bold px-1">{"Added video: Hello, World!"}</p>
                                </div>
                            </div>
                        </div>
                    </div>

                    // Part 2
                    <h1 class="text-white font-bold text-2xl py-4">
                        {"2) Run sp!next to cycle the queue."}
                    </h1>
                    <div class="bg-discord rounded-lg shadow-md ml-12 px-4 w-2/3">
                        <div class="flex py-4">
                            <img
                                class="inline-block rounded-full h-12 w-12"
                                src={"https://cdn.discordapp.com/avatars/290923752475066368/4921a5665c5320be55559d1a026fca68.webp?size=128"}
                                alt=""
                            />
                            <div class="inline-block px-3 w-5/6">
                                <h1 class="text-blue-400 font-semibold">{"ハーリさん (CF8)"}</h1>
                                <p class="text-white">{"sp!next"}</p>
                            </div>
                        </div>
                        <div class="flex py-4">
                            <img
                                class="inline-block rounded-full h-12 w-12"
                                src={"https://cdn.discordapp.com/avatars/585225058683977750/73628acbb1304b05c718f22a380767bd.png?size=128"}
                                alt=""
                            />
                            <div class="inline-block px-3 w-5/6">
                                <h1 class="text-blue-400 font-semibold">{"Spooderfy"}</h1>
                                <p class="text-white font-bold px-1">{"🎉 Moved to next video!"}</p>
                            </div>
                        </div>
                    </div>
                </div>
            }
        };

        html! {
            <div class="w-2/3 p-4">
                <div class="bg-discord-dark rounded-lg p-4">
                    {render}
                </div>
            </div>
        }
    }
}


/// A seek event payload.
#[derive(Serialize, Deserialize)]
struct SeekTo {
    /// The position of the track in seconds.
    pos: u32,
}


/// The finalised payload for sending a time check response, this allows
/// the gateway to calculate a average and emit to all players.
///
/// Why is this not just a SeekTo struct? -> Im not sure but it's likely going
/// to change as beta rolls out so id rather keep these separate for now.
#[derive(Serialize)]
struct SubmitTimeCheck {
    /// The position of the track in seconds.
    pos: u32,
}


/// The video player properties that can be specified.
#[derive(Properties, Clone)]
pub struct PlayerProperties {
    /// The video source
    src: String,

    /// The WS handle to subscribe and register event listeners.
    ws: WsHandler,

    /// The room id of the current url.
    room_id: String,
}


/// All video player event spec
pub enum VideoEvent {
    /// Starts playing the video due to the user clicking the play button.
    Play,

    /// Pauses the video from a user event / button click.
    Pause,

    /// Mutes the video, this also hides volume controls.
    Mute,

    /// UnMutes the video and shows the audio controls again.
    UnMute,

    /// Maximises the video player.
    FullScreen,

    /// Invokes a callback to take a snapshot of the position of the seekbar
    /// in order to send to the websocket.
    ShouldSend,

    /// Called when ever the mouse is clicking on the input and adjusting
    /// the value done on a range of 0 - 1000
    UpdateSeek(InputData),

    /// Called when ever the video time is polled and the time is changed,
    /// this has a default resolution of 1s as that's all the interval fires at.
    UpdatePos,

    /// Adjusts the volume from 0 - 100 based on what the slider value is.
    UpdateVol(InputData),
}


/// The Websocket type events that can be invoked via the websocket to
/// control the media player.
pub enum VideoWebsocketEvent {
    /// Plays the video if the user has clicked the button first.
    Play,

    /// Pauses the video if its not already paused.
    Pause,

    /// Seeks the player to a given position contained within the web ws
    /// message.
    Seek(WebsocketMessage),

    /// A synchronising callback to allow players to sync times with each other
    /// when a new client joins, this should not interrupt the running players
    /// however, instead it should just take a reference and echo it back.
    TimeCheck,
}


/// The two separate types of player events the video player can receive,
/// either from the websocket or from the video element itself.
pub enum VideoPlayerEvents {
    /// Websocket invoked events.
    Websocket(VideoWebsocketEvent),

    /// User invoked events.
    VideoEvent(VideoEvent),
}


/// The custom HTML5 video player this controls all custom components and
/// the player itself as well as its relevant JS bindings.
pub struct VideoPlayer {
    link: ComponentLink<Self>,
    player: video::Video,
    _ws: WsHandler,
    room_id: String,
    _task: IntervalTask,
    first_start: bool,
    ignore_time_check: bool,
}

impl Component for VideoPlayer {
    type Message = VideoPlayerEvents;
    type Properties = PlayerProperties;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let video_player = video::Video::new(false, props.src);

        let ticker = link.callback(
            |_| VideoPlayerEvents::VideoEvent(VideoEvent::UpdatePos)
        );

        let task = IntervalService::spawn(
            Duration::from_secs(1),
            ticker
        );

        let on_play = link.callback(
            |_| VideoPlayerEvents::Websocket(VideoWebsocketEvent::Play)
        );

        let on_pause = link.callback(
            |_| VideoPlayerEvents::Websocket(VideoWebsocketEvent::Pause)
        );

        let on_seek = link.callback(
            |event| VideoPlayerEvents::Websocket(VideoWebsocketEvent::Seek(event))
        );

        let on_time_check = link.callback(
            |_| VideoPlayerEvents::Websocket(VideoWebsocketEvent::TimeCheck)
        );


        let ws = props.ws;
        ws.subscribe_to_message(
            settings::PLAYER_ID,
            opcodes::OP_PLAY,
            on_play
        );

        ws.subscribe_to_message(
            settings::PLAYER_ID,
            opcodes::OP_PAUSE,
            on_pause
        );

        ws.subscribe_to_message(
            settings::PLAYER_ID,
            opcodes::OP_SEEK,
            on_seek
        );

        ws.subscribe_to_message(
            settings::PLAYER_ID,
            opcodes::OP_TIME_CHECK,
            on_time_check
        );

        Self {
            link,
            _ws: ws,
            room_id: props.room_id,
            player: video_player,
            _task: task,
            first_start: true,
            ignore_time_check: false,
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            VideoPlayerEvents::VideoEvent(event) => {
                self.on_video_event(event)
            },
            VideoPlayerEvents::Websocket(msg) => {
                self.on_ws_message(msg)
            },
        }

        true
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        if &self.player.src == &props.src {
            return false;
        }

        self.player.set_src(props.src);
        true
    }


    /// Renders the custom HTML player which has its own controls instead of
    /// the default player controls.
    fn view(&self) -> Html {
        let play_pause = if self.player.playing {
            let onclick = self.link.callback(
                |_| VideoPlayerEvents::VideoEvent(VideoEvent::Pause)
            );
            html!{
                <button onclick=onclick class="text-white cursor-pointer focus:outline-none mx-2 h-8 w-8">
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 9v6m4-6v6m7-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                </button>
            }
        } else {
            let onclick = self.link.callback(
                |_| VideoPlayerEvents::VideoEvent(VideoEvent::Play)
            );
            html!{
                <button onclick=onclick class="text-white cursor-pointer focus:outline-none mx-2 h-8 w-8">
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                </button>
            }
        };

        let mute = if self.player.muted {
            let onclick = self.link.callback(
                |_| VideoPlayerEvents::VideoEvent(VideoEvent::UnMute)
            );
            html!{
                <button onclick=onclick class="text-white cursor-pointer focus:outline-none mx-2 h-8 w-8">
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5.586 15H4a1 1 0 01-1-1v-4a1 1 0 011-1h1.586l4.707-4.707C10.923 3.663 12 4.109 12 5v14c0 .891-1.077 1.337-1.707.707L5.586 15z" clip-rule="evenodd" />
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2" />
                    </svg>
                </button>
            }
        } else {
            let onclick = self.link.callback(
                |_| VideoPlayerEvents::VideoEvent(VideoEvent::Mute)
            );
            let update_cb = self.link.callback(
                |e| VideoPlayerEvents::VideoEvent(VideoEvent::UpdateVol(e))
            );
            html!{
                <div class="inline-block flex justify-start items-center w-36">
                    <button onclick=onclick class="text-white cursor-pointer focus:outline-none mx-2 h-8 w-8">
                        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.536 8.464a5 5 0 010 7.072m2.828-9.9a9 9 0 010 12.728M5.586 15H4a1 1 0 01-1-1v-4a1 1 0 011-1h1.586l4.707-4.707C10.923 3.663 12 4.109 12 5v14c0 .891-1.077 1.337-1.707.707L5.586 15z" />
                        </svg>
                    </button>
                    <input
                        class="focus:outline-none slider h-1 w-24"
                        type="range"
                        min="0"
                        max="100"
                        value=self.player.volume
                        oninput=update_cb
                    />
                </div>
            }
        };

        let fullscreen = {
            let onclick = self.link.callback(
                |_| VideoPlayerEvents::VideoEvent(VideoEvent::FullScreen)
            );
            html! {
                <button onclick=onclick class="float-right text-white cursor-pointer focus:outline-none mx-2 h-8 w-8">
                   <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4" />
                   </svg>
                </button>
            }
        };

        let pct_str = format!("{:.1}%", self.player.pct_pos);
        let value_str = format!("{:.0}", self.player.pct_pos * 10f32);

        let should_send = self.link.callback(
            |_| VideoPlayerEvents::VideoEvent(VideoEvent::ShouldSend)
        );
        let update_cb = self.link.callback(
            |e| VideoPlayerEvents::VideoEvent(VideoEvent::UpdateSeek(e))
        );

        let seek_bar = html! {
            <>
                <div class="relative h-2 mb-2">
                    <div class="bg-blue-600 h-2" style={format!("width: {}", pct_str)}></div>
                    <input
                        class="absolute top-0 slider bg-transparent focus:outline-none h-2 w-full"
                        type="range"
                        min="0"
                        max="1000"
                        onmouseup=should_send
                        oninput=update_cb
                        value={ value_str }
                    />
                </div>
            </>
        };


        let player_controls = html! {
            <div class="bg-black bg-opacity-75 rounded-b-lg absolute inset-x-0 bottom-0 h-15">
                { seek_bar }
                <div class="flex justify-between mt-1 mb-2">
                    <div class="flex justify-start items-center">
                        { play_pause }
                        { mute }
                    </div>
                    <div class="flex justify-end items-center">
                        { fullscreen }
                    </div>
                </div>
            </div>
        };

        html! {
            <div class="flex justify-center w-full">
                <div id="video-container" class="relative w-full">
                    { self.player.view() }
                    { player_controls }
                </div>
            </div>
        }
    }
}

impl VideoPlayer {
    /// Invokes the relevant callbacks and code on a given video event,
    /// these are only invoked from the user on the website clicking something
    /// as an action rather than the websocket invoking something.
    fn on_video_event(&mut self, event: VideoEvent) {
        match event {
            VideoEvent::Play => {
                if self.first_start {
                    let msg = WrappingWsMessage {
                        opcode: opcodes::OP_TIME_CHECK,
                        payload: None,
                    };

                    start_future(emit_event(
                        self.room_id.clone(),
                        msg,
                    ));

                    let msg = WrappingWsMessage {
                        opcode: opcodes::OP_PAUSE,
                        payload: None,
                    };

                    start_future(emit_event(
                        self.room_id.clone(),
                        msg,
                    ));

                    self.player.play();
                    self.player.pause();

                    self.first_start = false;
                    self.ignore_time_check = true;
                    return;
                }

                self.on_play();
            },
            VideoEvent::Pause => {
                self.on_pause();
            },
            VideoEvent::Mute => {
                self.player.mute();
            },
            VideoEvent::UnMute => {
                self.player.unmute();
            },
            VideoEvent::FullScreen => {
                self.player.toggle_fullscreen();
            },
            VideoEvent::ShouldSend => {
                let pos = self.player.get_pos();
                self.on_seek_complete(pos);
            },
            VideoEvent::UpdateSeek(event) => {
                let pos = event.value.parse::<u32>().unwrap();
                self.player.pct_pos = pos as f32 / 10f32;

                let dur = self.player.get_duration();
                let seek_to_mod = self.player.pct_pos / 100f32;
                let seek_to = (dur as f32 * seek_to_mod) as u32;

                self.player.seek(seek_to);
            },
            VideoEvent::UpdatePos => {
                self.player.update_pos();
            },
            VideoEvent::UpdateVol(e) => {
                let vol = e.value.parse::<u32>().unwrap();
                self.player.set_vol(vol);
            },
        };
    }

    /// Matches a given websocket event relevant to the video player
    /// and invokes it's relevant callbacks.
    fn on_ws_message(&mut self, msg: VideoWebsocketEvent) {
        match msg {
            VideoWebsocketEvent::Play => {
                if self.first_start {
                    return;
                }

                self.player.play()
            },
            VideoWebsocketEvent::Pause => {
                if self.first_start {
                    return;
                }

                self.player.pause()
            },
            VideoWebsocketEvent::Seek(value) => {
                if self.first_start {
                    return;
                }

                let payload: SeekTo = value.unwrap_and_into().unwrap();
                self.player.seek(payload.pos)
            },
            VideoWebsocketEvent::TimeCheck => {
                if self.ignore_time_check {
                    self.ignore_time_check = false;
                    return;
                }

                let pos = self.player.get_pos();
                let url = settings::get_time_check_url(&self.room_id);
                send_post(url, SubmitTimeCheck{ pos })
            }
        };
    }

    /// Invoked when the player has seeked to a location and the mouse has
    /// been released, this allows the socket to only send one seek request
    /// per selection and also to minimise socket traffic.
    fn on_seek_complete(&mut self, pos: u32) {
        let seek_to = SeekTo { pos };
        let msg = WrappingWsMessage {
            opcode: opcodes::OP_SEEK,
            payload: Some(serde_json::to_value(seek_to).unwrap())
        };

        start_future(emit_event(self.room_id.clone(), msg));
    }

    /// Invoked when the play button is clicked on the player, this is not
    /// invoked when it is called from rust however.
    fn on_play(&mut self) {
        let msg = WrappingWsMessage {
            opcode: opcodes::OP_PLAY,
            payload: None
        };

        start_future(emit_event(self.room_id.clone(), msg));
    }

    /// Invoked when the pause button is clicked on the player, this is not
    /// invoked when it is called from rust however.
    fn on_pause(&mut self) {
        let msg = WrappingWsMessage {
            opcode: opcodes::OP_PAUSE,
            payload: None
        };

        start_future(emit_event(self.room_id.clone(), msg));
    }
}
