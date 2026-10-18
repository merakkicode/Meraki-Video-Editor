use crate::transport::Transport;
use std::{
    sync::mpsc::{self, SyncSender},
    thread,
};
use tauri::{
    http::{Request, Response},
    UriSchemeResponder,
};
type Job = (Request<Vec<u8>>, UriSchemeResponder);
pub struct FrameServer {
    sender: SyncSender<Job>,
}
impl FrameServer {
    pub fn spawn(transport: Transport) -> std::io::Result<Self> {
        let (tx, rx) = mpsc::sync_channel::<Job>(2);
        thread::Builder::new()
            .name("meraki-frame-protocol".into())
            .spawn(move || {
                while let Ok((request, responder)) = rx.recv() {
                    responder.respond(response(&transport, &request));
                }
            })?;
        Ok(Self { sender: tx })
    }
    pub fn serve(&self, request: Request<Vec<u8>>, responder: UriSchemeResponder) {
        if let Err(e) = self.sender.try_send((request, responder)) {
            let (_, responder) = match e {
                mpsc::TrySendError::Full(job) | mpsc::TrySendError::Disconnected(job) => job,
            };
            responder.respond(
                Response::builder()
                    .status(429)
                    .body(Vec::<u8>::new())
                    .unwrap(),
            );
        }
    }
}
fn response(transport: &Transport, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let origin = request
        .headers()
        .get("Origin")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("http://tauri.localhost");
    if ![
        "http://tauri.localhost",
        "https://tauri.localhost",
        "tauri://localhost",
        "http://127.0.0.1:1420",
    ]
    .contains(&origin)
    {
        return Response::builder().status(403).body(vec![]).unwrap();
    }
    let base = Response::builder()
        .header("Access-Control-Allow-Origin", origin)
        .header(
            "Access-Control-Expose-Headers",
            "X-Generation,X-Sequence,X-Width,X-Height,X-Stride,X-Blank",
        )
        .header("Cache-Control", "no-store")
        .header("Content-Type", "application/octet-stream");
    if request.method() != "GET" {
        return base.status(405).body(vec![]).unwrap();
    }
    let generation = request
        .uri()
        .path()
        .strip_prefix("/frame/")
        .and_then(|s| s.parse::<u64>().ok());
    let Some(p) = generation.and_then(|g| transport.presented(g)) else {
        return base.status(204).body(vec![]).unwrap();
    };
    let base = base
        .header("X-Generation", p.generation.to_string())
        .header("X-Sequence", p.sequence.to_string());
    match p.frame {
        Some(f) => base
            .header("X-Width", f.width.to_string())
            .header("X-Height", f.height.to_string())
            .header("X-Stride", f.stride.to_string())
            .header("X-Blank", "0")
            .body((*f.pixels).clone())
            .unwrap(),
        None => base.header("X-Blank", "1").body(vec![]).unwrap(),
    }
}
