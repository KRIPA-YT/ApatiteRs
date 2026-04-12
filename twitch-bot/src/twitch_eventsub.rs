use futures::StreamExt;
use serde_json::Value;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

pub type EventSubSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub async fn connect_eventsub() -> Option<(EventSubSocket, String)> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
    let (mut ws_stream, _) = connect_async("wss://eventsub.wss.twitch.tv/ws")
        .await
        .expect("Failed to connect to EventSub");

    println!("Connected to EventSub WebSocket");

    while let Some(msg) = ws_stream.next().await {
        let msg = msg.unwrap();
        if msg.is_text() {
            let data: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();

            if let Some("session_welcome") = data["metadata"]["message_type"].as_str() {
                let session_id = data["payload"]["session"]["id"]
                    .as_str()
                    .unwrap()
                    .to_string();

                println!("Session ID: {}", session_id);
                return Some((ws_stream, session_id));
            }
        }
    }
    None
}
