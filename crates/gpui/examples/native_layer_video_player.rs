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
    NativeLayerId, NativeLayerZOrder, ParentElement, Render, SharedString, Styled, Window,
    WindowBounds, WindowOptions,
};
use gpui_platform::application;
use std::ffi::c_void;

#[cfg(target_os = "macos")]
use objc::{class, msg_send, runtime::Object, sel, sel_impl};

#[cfg(target_os = "macos")]
type Id = *mut Object;

const SAMPLE_VIDEO_URL: &str =
    "https://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4";

struct VideoPlayer {
    layer_id: Option<NativeLayerId>,
    is_playing: bool,
    status: SharedString,
    #[cfg(target_os = "macos")]
    player: Id,
}

impl VideoPlayer {
    #[cfg(target_os = "macos")]
    fn new(window: &mut Window) -> Self {
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

            Self {
                layer_id: Some(layer_id),
                is_playing: false,
                status: "Ready to play".into(),
                player,
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn new(_window: &mut Window) -> Self {
        Self {
            layer_id: None,
            is_playing: false,
            status: "Video playback is only supported on macOS".into(),
        }
    }

    #[cfg(target_os = "macos")]
    fn toggle_playback(&mut self) {
        unsafe {
            if self.is_playing {
                let _: () = msg_send![self.player, pause];
                self.status = "Paused".into();
            } else {
                let _: () = msg_send![self.player, play];
                self.status = "Playing".into();
            }
            self.is_playing = !self.is_playing;
        }
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
            |window, cx| cx.new(|_| VideoPlayer::new(window)),
        )
        .expect("Failed to open window");
        cx.activate(true);
    });
}
