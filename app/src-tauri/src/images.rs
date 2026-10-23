//! The card images kept on disk: the interface asks for them through the `cardimg` protocol
//! (images.ts of the app) instead of Scryfall's CDN. An image already kept is read from the
//! disk; any other is fetched from the CDN, kept for next time, then shown. The cards seen
//! once show offline, and faster after a restart. The WebView's own HTTP cache did some of
//! this, but empties itself when it likes.
//!
//! The cache is kept under `MAX_BYTES`: past it, the oldest images go (at launch).

use std::fs;
use std::path::{Path, PathBuf};
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
    if !is_card_image_path(path) {
        return respond(StatusCode::NOT_FOUND, "text/plain", b"not a card image".to_vec());
    }
    let content_type = if path.ends_with(".png") { "image/png" } else { "image/jpeg" };
    let file = path.split('/').fold(dir.to_path_buf(), |p, part| p.join(part));
    if let Ok(bytes) = fs::read(&file) {
        return respond(StatusCode::OK, content_type, bytes);
    }
    match card_image(path) {
        Ok(bytes) => {
            // Written beside, then renamed: a half-written image is never read.
            if let Some(parent) = file.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let tmp = file.with_extension(format!("{}.part", std::process::id()));
            if fs::write(&tmp, &bytes).is_ok() && fs::rename(&tmp, &file).is_err() {
                let _ = fs::remove_file(&tmp);
            }
            respond(StatusCode::OK, content_type, bytes)
        }
        // Offline, say: the card shows without its image, as before.
        Err(e) => respond(StatusCode::BAD_GATEWAY, "text/plain", e.to_string().into_bytes()),
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
