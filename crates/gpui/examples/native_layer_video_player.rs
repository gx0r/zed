//! Example demonstrating native layer embedding for video playback.
//!
//! This example shows how to embed an AVPlayerLayer within a GPUI window
//! using the native_layer API. The video is positioned using GPUI's flexbox
//! layout system.
//!
//! Run with: `cargo run -p gpui --example native_layer_video_player`
//!
//! Note: This example only works on macOS.

#[cfg(target_os = "macos")]
#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {}

use gpui::{
    div, native_layer_element, prelude::*, px, rgb, size, App, Bounds, Context, NativeLayerConfig,
    NativeLayerId, NativeLayerZOrder, ParentElement, Render, SharedString, Styled, Task, Window,
    WindowBounds, WindowOptions,
};
use gpui_platform::application;
use std::ffi::c_void;
use std::time::Duration;

#[cfg(target_os = "macos")]
use objc::{class, msg_send, runtime::Object, sel, sel_impl};

#[cfg(target_os = "macos")]
type Id = *mut Object;

// Apple's reference HLS stream, used by their own AVFoundation samples. Third-party
// sample URLs tend to rot; this one is tied to the framework being demonstrated.
const SAMPLE_VIDEO_URL: &str =
    "https://devstreaming-cdn.apple.com/videos/streaming/examples/img_bipbop_adv_example_fmp4/master.m3u8";

// `AVPlayerItemStatus`
#[cfg(target_os = "macos")]
const ITEM_STATUS_READY_TO_PLAY: i64 = 1;
#[cfg(target_os = "macos")]
const ITEM_STATUS_FAILED: i64 = 2;

struct VideoPlayer {
    layer_id: Option<NativeLayerId>,
    is_playing: bool,
    has_played: bool,
    status: SharedString,
    #[cfg(target_os = "macos")]
    player: Id,
    #[cfg(target_os = "macos")]
    _poll_status: Task<()>,
}

impl VideoPlayer {
    #[cfg(target_os = "macos")]
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        unsafe {
            // Create NSURL from string
            let url_string = ns_string(SAMPLE_VIDEO_URL);
            let url: Id = msg_send![class!(NSURL), URLWithString: url_string];

            // Create AVPlayer with the URL
            let player: Id = msg_send![class!(AVPlayer), playerWithURL: url];
            let _: () = msg_send![player, retain];

            // Create AVPlayerLayer
            let player_layer: Id = msg_send![class!(AVPlayerLayer), playerLayerWithPlayer: player];
            let _: () = msg_send![player_layer, retain];

            // Configure the layer to maintain aspect ratio
            let resize_aspect = ns_string("AVLayerVideoGravityResizeAspect");
            let _: () = msg_send![player_layer, setVideoGravity: resize_aspect];

            // Add the layer to the window (AboveContent so video renders on top of GPUI)
            let layer_id = window.add_native_layer(
                player_layer as *mut c_void,
                NativeLayerConfig {
                    z_order: NativeLayerZOrder::AboveContent,
                    hidden: false,
                    opacity: 1.0,
                    manual_visibility: false,
                },
            );

            // An AVPlayerItem loads asynchronously and reports failures only on itself,
            // so poll it. Without this a failed load is indistinguishable from a working
            // player pointed at a black frame.
            let poll_status = cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(250))
                        .await;
                    let updated = this.update(cx, |this, cx| {
                        this.refresh_status();
                        cx.notify();
                    });
                    if updated.is_err() {
                        break;
                    }
                }
            });

            Self {
                layer_id: Some(layer_id),
                is_playing: false,
                has_played: false,
                status: "Loading\u{2026}".into(),
                player,
                _poll_status: poll_status,
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            layer_id: None,
            is_playing: false,
            has_played: false,
            status: "Video playback is only supported on macOS".into(),
        }
    }

    /// Mirrors the real player state into the UI, so a failed load surfaces as text
    /// rather than as an indefinitely black layer.
    #[cfg(target_os = "macos")]
    fn refresh_status(&mut self) {
        unsafe {
            let item: Id = msg_send![self.player, currentItem];
            if item.is_null() {
                self.is_playing = false;
                self.status = "No media".into();
                return;
            }

            let rate: f32 = msg_send![self.player, rate];
            self.is_playing = rate != 0.0;

            let item_status: i64 = msg_send![item, status];
            self.status = match item_status {
                ITEM_STATUS_FAILED => {
                    let error: Id = msg_send![item, error];
                    let description: Id = if error.is_null() {
                        std::ptr::null_mut()
                    } else {
                        msg_send![error, localizedDescription]
                    };
                    match ns_string_to_string(description) {
                        Some(description) => format!("Error: {description}").into(),
                        None => "Error: playback failed".into(),
                    }
                }
                ITEM_STATUS_READY_TO_PLAY if self.is_playing => "Playing".into(),
                ITEM_STATUS_READY_TO_PLAY if self.has_played => "Paused".into(),
                ITEM_STATUS_READY_TO_PLAY => "Ready to play".into(),
                _ => "Loading\u{2026}".into(),
            };
        }
    }

    #[cfg(target_os = "macos")]
    fn toggle_playback(&mut self) {
        unsafe {
            if self.is_playing {
                let _: () = msg_send![self.player, pause];
            } else {
                let _: () = msg_send![self.player, play];
                self.has_played = true;
            }
        }
        self.refresh_status();
    }

    #[cfg(not(target_os = "macos"))]
    fn toggle_playback(&mut self) {}
}

#[cfg(target_os = "macos")]
impl Drop for VideoPlayer {
    fn drop(&mut self) {
        unsafe {
            let _: () = msg_send![self.player, pause];
            let _: () = msg_send![self.player, release];
        }
    }
}

impl Render for VideoPlayer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e1e1e))
            .child(
                // Title bar
                div()
                    .flex()
                    .justify_center()
                    .items_center()
                    .h(px(40.0))
                    .bg(rgb(0x2d2d2d))
                    .text_color(rgb(0xffffff))
                    .child("Native Layer Video Player Example"),
            )
            .child(
                // Video area
                div()
                    .flex()
                    .flex_1()
                    .justify_center()
                    .items_center()
                    .bg(rgb(0x000000))
                    .child(if let Some(layer_id) = self.layer_id {
                        // The native layer element positions the AVPlayerLayer
                        // within GPUI's flexbox layout
                        native_layer_element(layer_id)
                            .w(px(640.0))
                            .h(px(360.0))
                            .into_any_element()
                    } else {
                        div()
                            .text_color(rgb(0x888888))
                            .child("Video not available on this platform")
                            .into_any_element()
                    }),
            )
            .child(
                // Controls bar
                div()
                    .flex()
                    .justify_center()
                    .items_center()
                    .gap_4()
                    .h(px(60.0))
                    .bg(rgb(0x2d2d2d))
                    .child(
                        div()
                            .id("play-button")
                            .flex()
                            .justify_center()
                            .items_center()
                            .px(px(20.0))
                            .py(px(10.0))
                            .bg(rgb(0x4a9eff))
                            .hover(|s| s.bg(rgb(0x3a8eef)))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .text_color(rgb(0xffffff))
                            .child(if self.is_playing { "Pause" } else { "Play" })
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.toggle_playback();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_color(rgb(0xaaaaaa))
                            .child(self.status.clone()),
                    ),
            )
    }
}

#[cfg(target_os = "macos")]
unsafe fn ns_string_to_string(string: Id) -> Option<String> {
    if string.is_null() {
        return None;
    }
    unsafe {
        let c_str: *const std::os::raw::c_char = msg_send![string, UTF8String];
        if c_str.is_null() {
            return None;
        }
        Some(std::ffi::CStr::from_ptr(c_str).to_string_lossy().into_owned())
    }
}

#[cfg(target_os = "macos")]
fn ns_string(s: &str) -> Id {
    use std::ffi::CString;
    let c_str = CString::new(s).expect("CString::new failed");
    unsafe { msg_send![class!(NSString), stringWithUTF8String: c_str.as_ptr()] }
}

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(800.0), px(600.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| VideoPlayer::new(window, cx)),
        )
        .expect("Failed to open window");
        cx.activate(true);
    });
}
