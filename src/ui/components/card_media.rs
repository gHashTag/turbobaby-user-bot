//! Shared product-card media area (image / inline autoplay video / emoji).
//!
//! Extracted so strain, accessory and set cards all render the SAME media block
//! the owner asked for: a muted, looping, inline-autoplaying `<video>` preview
//! (with the product photo as poster) when a video exists — otherwise the image,
//! otherwise an emoji.
//!
//! Badges are overlaid by the caller via `children` (this div is the
//! `position:relative` anchor for their absolute positioning).
//!
//! The full-screen [`VideoModal`] is NOT rendered here: the card root carries
//! `overflow:hidden` and `will-change:transform` (`.comet-card`), which makes it a
//! containing block for `position:fixed` descendants and clips them to the card
//! rectangle.  Callers must hoist `VideoModal` as a sibling of the card and use
//! `on_video_click` to open it.

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
    /// Called when the user taps the video preview area.
    #[props(default)]
    on_video_click: EventHandler<()>,
    children: Element,
) -> Element {
    let img = image_url.unwrap_or_default();
    let has_image = is_media_url(&img);
    let vid = video_url.unwrap_or_default();
    let has_video = is_media_url(&vid);

    rsx! {
        div { style: "width:100%;aspect-ratio:4/3;background:linear-gradient(135deg,#1a1a2e,#16213e);display:flex;align-items:center;justify-content:center;position:relative;overflow:hidden;",
            if has_video {
                video {
                    style: "width:100%;height:100%;object-fit:cover;display:block;",
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
                }
                // Transparent overlay catches taps and prevents the native video
                // element from intercepting touches (iOS sometimes shows its own
                // controls or pauses playback on tap).
                div {
                    style: "position:absolute;inset:0;z-index:3;cursor:pointer;",
                    onclick: move |e: Event<MouseData>| { e.stop_propagation(); on_video_click.call(()); }
                }
            } else if has_image {
                img {
                    src: "{img}",
                    alt: "{alt}",
                    loading: "lazy",
                    style: "width:100%;height:100%;object-fit:cover;display:block;"
                }
            } else {
                span { style: "font-size:48px;", "{emoji}" }
            }
            {children}
        }
    }
}
