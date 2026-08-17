use crate::api::AppState;
use crate::geometry::Frame;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State as AxState;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;

pub fn frame_to_json(frame: &Frame) -> String {
    let pixels: Vec<[u8; 3]> = frame.iter().map(|p| [p.r, p.g, p.b]).collect();
    json!({ "pixels": pixels }).to_string()
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
) -> impl IntoResponse {
    // Validate Origin on the upgrade (browser-set, unspoofable by page JS).
    let origin = headers.get("origin").and_then(|v| v.to_str().ok());
    let host = headers.get("host").and_then(|v| v.to_str().ok());
    if !st.security.origin_ok(origin) || !st.security.host_ok(host) {
        return StatusCode::FORBIDDEN.into_response();
    }
    ws.on_upgrade(move |socket| client_loop(socket, st))
}

async fn client_loop(mut socket: WebSocket, st: AppState) {
    // 1. Send the current frame immediately (solid mode is otherwise idle).
    let current = crate::effects::render_frame(&st.engine.current(), 0);
    if socket
        .send(Message::Text(frame_to_json(&current)))
        .await
        .is_err()
    {
        return;
    }
    // 2. Stream frames; drop this client on lag (never back-pressure the engine).
    let mut rx = st.engine.subscribe_frames();
    let mut shutdown = st.shutdown.clone();
    // If shutdown is already underway, don't start streaming.
    if *shutdown.borrow() {
        return;
    }
    loop {
        tokio::select! {
            // On shutdown, exit promptly so the upgraded connection closes and
            // hyper's graceful shutdown can complete (see AppState::shutdown).
            _ = shutdown.changed() => break,
            recv = rx.recv() => match recv {
                Ok(frame) => {
                    if socket.send(Message::Text(frame_to_json(&frame))).await.is_err() {
                        break; // client gone
                    }
                }
                Err(RecvError::Lagged(_)) => continue, // slow client: skip missed frames
                Err(RecvError::Closed) => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgb;
    use crate::geometry::BLACK_FRAME;

    #[test]
    fn frame_serializes_to_flat_rgb_triples() {
        let mut frame = BLACK_FRAME;
        frame[0] = Rgb { r: 1, g: 2, b: 3 };
        let json = frame_to_json(&frame);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["pixels"].as_array().unwrap().len(), 32);
        assert_eq!(v["pixels"][0][0], 1);
        assert_eq!(v["pixels"][0][2], 3);
    }
}
