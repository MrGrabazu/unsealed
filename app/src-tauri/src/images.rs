//! The card images kept on disk: the interface asks for them through the `cardimg` protocol
//! (images.ts of the app) instead of Scryfall's CDN. An image already kept is read from the
//! disk; any other is fetched from the CDN, kept for next time, then shown. The cards seen
//! once show offline, and faster after a restart. The WebView's own HTTP cache did some of
//! this, but empties itself when it likes.
//!
//! The cache is kept under `MAX_BYTES`: past it, the oldest images go (at launch).

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, OnceLock};
use tauri::http::{header, Request, Response, StatusCode};
use tauri::UriSchemeResponder;
use unsealed_core::scryfall::{card_image, is_card_image_path};

/// The protocol's name: `http://cardimg.localhost/<path>` on Windows, `cardimg://localhost/<path>`
/// elsewhere, the path being the image's on the CDN ("normal/front/a/b/<id>.jpg").
pub const SCHEME: &str = "cardimg";
/// At most this much on disk (a "normal" image weighs about 70 KB: some 20,000 of them).
const MAX_BYTES: u64 = 1536 << 20;
/// What a pruning brings the cache down to, not to prune again at each launch.
const PRUNE_TO: u64 = MAX_BYTES / 4 * 3;

fn respond(status: StatusCode, content_type: &str, body: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        // The 3D opening loads its textures as cross-origin images.
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        // An image never changes at its path: the WebView may keep it as long as it likes.
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .body(body)
        .expect("a valid response")
}

/// Answers a request of the protocol, from the disk or the CDN, on a thread of its own: the
/// interface is never kept waiting by a download.
pub fn handle(dir: PathBuf, request: Request<Vec<u8>>, responder: UriSchemeResponder) {
    let path = request.uri().path().trim_start_matches('/').to_string();
    tauri::async_runtime::spawn_blocking(move || responder.respond(image(&dir, &path)));
}

fn image(dir: &Path, path: &str) -> Response<Vec<u8>> {
    if let Some(source) = path.strip_prefix(THUMB_PREFIX) {
        return thumbnail(dir, path, source);
    }
    if !is_card_image_path(path) {
        return respond(StatusCode::NOT_FOUND, "text/plain", b"not a card image".to_vec());
    }
    let content_type = if path.ends_with(".png") { "image/png" } else { "image/jpeg" };
    let file = file_of(dir, path);
    if let Ok(bytes) = fs::read(&file) {
        return respond(StatusCode::OK, content_type, bytes);
    }
    match fetch_and_keep(&file, path) {
        Ok(bytes) => respond(StatusCode::OK, content_type, bytes),
        // Offline, say: the card shows without its image, as before.
        Err(e) => respond(StatusCode::BAD_GATEWAY, "text/plain", e),
    }
}

/// Where an image is kept: its CDN path under the cache.
fn file_of(dir: &Path, path: &str) -> PathBuf {
    path.split('/').fold(dir.to_path_buf(), |p, part| p.join(part))
}

/// Downloads an image from the CDN and keeps it in `file`.
fn fetch_and_keep(file: &Path, path: &str) -> Result<Vec<u8>, Vec<u8>> {
    let bytes = card_image(path).map_err(|e| e.to_string().into_bytes())?;
    keep(file, &bytes);
    Ok(bytes)
}

/// Keeps an image in `file`: written beside, then renamed, so a half-written image is never read.
fn keep(file: &Path, bytes: &[u8]) {
    if let Some(parent) = file.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let tmp = file.with_extension(format!("{}.{:?}.part", std::process::id(), std::thread::current().id()).replace(['(', ')'], ""));
    if fs::write(&tmp, bytes).is_ok() && fs::rename(&tmp, file).is_err() {
        let _ = fs::remove_file(&tmp);
    }
}

/// "thumb/<path>": the image at `path` shrunk to `THUMB_WIDTH` pixels wide, for the lists whose
/// cards (or set arts) are shown smaller than that (`fittedImage` of images.ts). Scryfall has
/// nothing between "small" (146 px) and "normal" (488 px); decoded, an image weighs its pixels,
/// so a grid of "normal" images held over a gigabyte. Made from the image kept (fetched if
/// need be), never cropped nor altered but in size, then kept beside the others.
const THUMB_PREFIX: &str = "thumb/";
const THUMB_WIDTH: u32 = 320;
const THUMB_QUALITY: u8 = 88;

fn thumbnail(dir: &Path, path: &str, source: &str) -> Response<Vec<u8>> {
    if !is_card_image_path(source) || !source.ends_with(".jpg") {
        return respond(StatusCode::NOT_FOUND, "text/plain", b"not a card image".to_vec());
    }
    let file = file_of(dir, path);
    if let Ok(bytes) = fs::read(&file) {
        return respond(StatusCode::OK, "image/jpeg", bytes);
    }
    let source_file = file_of(dir, source);
    let full = match fs::read(&source_file) {
        Ok(bytes) => bytes,
        Err(_) => match fetch_and_keep(&source_file, source) {
            Ok(bytes) => bytes,
            Err(e) => return respond(StatusCode::BAD_GATEWAY, "text/plain", e),
        },
    };
    match shrink(&full) {
        Ok(small) => {
            keep(&file, &small);
            respond(StatusCode::OK, "image/jpeg", small)
        }
        // An image that does not decode is shown as it is.
        Err(_) => respond(StatusCode::OK, "image/jpeg", full),
    }
}

/// A JPEG image shrunk to `THUMB_WIDTH` pixels wide (as it is when no wider).
fn shrink(bytes: &[u8]) -> Result<Vec<u8>, image::ImageError> {
    let picture = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg)?;
    if picture.width() <= THUMB_WIDTH {
        return Ok(bytes.to_vec());
    }
    let height = (u64::from(picture.height()) * u64::from(THUMB_WIDTH) / u64::from(picture.width())).max(1) as u32;
    let small = picture.resize_exact(THUMB_WIDTH, height, image::imageops::FilterType::CatmullRom).to_rgb8();
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, THUMB_QUALITY).encode_image(&small)?;
    Ok(out)
}

/// Threads downloading images ahead of time: few, so that the images an opening asks for
/// right now (`handle`) are not kept waiting behind them.
const PREFETCH_THREADS: usize = 3;

/// The images to download ahead of time, oldest asked first, and every path asked this
/// session (asked again, a path is skipped).
#[derive(Default)]
struct Queue {
    paths: VecDeque<(PathBuf, String)>,
    asked: HashSet<String>,
}

struct Prefetcher {
    queue: Mutex<Queue>,
    ready: Condvar,
}

fn prefetcher() -> &'static Prefetcher {
    static PREFETCHER: OnceLock<Prefetcher> = OnceLock::new();
    PREFETCHER.get_or_init(|| {
        for _ in 0..PREFETCH_THREADS {
            std::thread::spawn(prefetch_worker);
        }
        Prefetcher { queue: Mutex::new(Queue::default()), ready: Condvar::new() }
    })
}

fn prefetch_worker() {
    let p = prefetcher();
    loop {
        let (file, path) = {
            let mut queue = p.queue.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                match queue.paths.pop_front() {
                    Some(next) => break next,
                    None => queue = p.ready.wait(queue).unwrap_or_else(|e| e.into_inner()),
                }
            }
        };
        if !file.exists() && fetch_and_keep(&file, &path).is_err() {
            // Offline, say: it may be asked again later.
            p.queue.lock().unwrap_or_else(|e| e.into_inner()).asked.remove(&path);
        }
    }
}

/// Downloads card images ahead of time, in the background (`paths` from the CDN, see
/// `Game::booster_image_paths`): those of the sealed boosters in stock, so that opening one
/// does not wait for its cards (Scryfall's CDN is far from Europe). Images already kept, or
/// already asked this session, are skipped.
pub fn prefetch(dir: &Path, paths: Vec<String>) {
    let p = prefetcher();
    let mut queue = p.queue.lock().unwrap_or_else(|e| e.into_inner());
    for path in paths.into_iter().filter(|path| is_card_image_path(path)) {
        let file = file_of(dir, &path);
        if !file.exists() && queue.asked.insert(path.clone()) {
            queue.paths.push_back((file, path));
        }
    }
    p.ready.notify_all();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jpeg(width: u32, height: u32) -> Vec<u8> {
        let picture = image::RgbImage::from_fn(width, height, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 90]));
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90).encode_image(&picture).unwrap();
        out
    }

    #[test]
    fn thumbnails_keep_the_aspect_of_the_image() {
        let small = image::load_from_memory(&shrink(&jpeg(488, 680)).unwrap()).unwrap();
        assert_eq!((small.width(), small.height()), (320, 445));
        // No wider than a thumbnail: as it is.
        let narrow = jpeg(146, 204);
        assert_eq!(shrink(&narrow).unwrap(), narrow);
    }

    #[test]
    fn thumbnails_are_kept_and_served() {
        let dir = std::env::temp_dir().join(format!("unsealed-thumbs-{}", std::process::id()));
        let source = "normal/front/a/b/ab000000-0000-0000-0000-000000000000.jpg";
        keep(&file_of(&dir, source), &jpeg(488, 680));
        let response = image(&dir, &format!("{THUMB_PREFIX}{source}"));
        assert_eq!(response.status(), StatusCode::OK);
        assert!(file_of(&dir, &format!("{THUMB_PREFIX}{source}")).is_file());
        assert_eq!(image(&dir, "thumb/../secret.jpg").status(), StatusCode::NOT_FOUND);
        let _ = fs::remove_dir_all(&dir);
    }
}

/// Keeps the cache under `MAX_BYTES`, removing the images kept longest ago. Run at launch, on
/// a thread of its own.
pub fn prune(dir: &Path) {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d).into_iter().flatten().flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                files.push((meta.modified().ok(), meta.len(), entry.path()));
            }
        }
    }
    let mut total: u64 = files.iter().map(|(_, len, _)| len).sum();
    if total <= MAX_BYTES {
        return;
    }
    files.sort_by_key(|(at, _, _)| *at);
    for (_, len, path) in files {
        if total <= PRUNE_TO {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
}
