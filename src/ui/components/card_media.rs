//! Shared product-card media area (image / inline autoplay video / emoji).
//!
//! Extracted so strain, accessory and set cards all render the SAME media block
//! the owner asked for: a muted, looping, inline-autoplaying `<video>` preview
//! (with the product photo as poster) when a video exists — tap it to open the
//! full-screen [`VideoModal`] — otherwise the image, otherwise an emoji.
//!
//! Badges are overlaid by the caller via `children` (this div is the
//! `position:relative` anchor for their absolute positioning).

use crate::ui::components::video_modal::VideoModal;
use dioxus::prelude::*;

/// Accept only http(s) / root-relative URLs (never `//host` protocol-relative).
fn is_media_url(u: &str) -> bool {
    u.starts_with("http://")
        || u.starts_with("https://")
        || (u.starts_with('/') && !u.starts_with("//"))
}

#[component]
pub fn CardMedia(
    image_url: Option<String>,
    video_url: Option<String>,
    emoji: String,
    alt: String,
    children: Element,
) -> Element {
    let mut show_video = use_signal(|| false);
    let img = image_url.unwrap_or_default();
    let has_image = is_media_url(&img);
    let vid = video_url.unwrap_or_default();
    let has_video = is_media_url(&vid);
    let vid_modal = vid.clone();

    rsx! {
        div { style: "width:100%;min-height:180px;background:linear-gradient(135deg,#1a1a2e,#16213e);display:flex;align-items:center;justify-content:center;position:relative;",
            if has_video {
                video {
                    style: "width:100%;height:auto;display:block;",
                    src: "{vid}",
                    "type": "video/mp4",
                    autoplay: true,
                    muted: true,
                    "loop": true,
                    // playsinline is REQUIRED for muted autoplay in mobile
                    // WebViews (Telegram/iOS); without it the video stays paused.
                    "playsinline": "true",
                    "webkit-playsinline": "true",
                    preload: "auto",
                    // poster = the product image until the first frame paints.
                    poster: if has_image { "{img}" } else { "" },
                    // cross-origin (S3/Railway bucket) videos need this to paint.
                    crossorigin: "anonymous",
                    onclick: move |e: Event<MouseData>| { e.stop_propagation(); show_video.set(true); }
                }
            } else if has_image {
                img {
                    src: "{img}",
                    alt: "{alt}",
                    loading: "lazy",
                    style: "width:100%;height:auto;object-fit:contain;display:block;"
                }
            } else {
                span { style: "font-size:48px;", "{emoji}" }
            }
            {children}
            if show_video() {
                VideoModal { url: vid_modal.clone(), on_close: move |_| show_video.set(false) }
            }
        }
    }
}
