use apatite_api::twitch_api::TwitchAPIError;
use futures::StreamExt;
use serde_json::Value;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

pub type EventSubSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub async fn connect_eventsub() -> Result<(EventSubSocket, String), TwitchAPIError> {
    #[allow(clippy::expect_used)]
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
    let (mut ws_stream, _) = connect_async("wss://eventsub.wss.twitch.tv/ws")
        .await
        .map_err(|_| TwitchAPIError::RequestError)?;

    println!("Connected to EventSub WebSocket");

    while let Some(msg) = ws_stream.next().await {
        let msg = msg.map_err(|_| TwitchAPIError::ResponseError)?;
        if msg.is_text() {
            let data: Value =
                serde_json::from_str(msg.to_text().map_err(|_| TwitchAPIError::ResponseError)?)
                    .map_err(|_| TwitchAPIError::ParseError)?;

            if let Some("session_welcome") = data["metadata"]["message_type"].as_str() {
                let session_id = data["payload"]["session"]["id"]
                    .as_str()
                    .ok_or(TwitchAPIError::ParseError)?
                    .to_string();

                println!("Session ID: {}", session_id);
                return Ok((ws_stream, session_id));
            }
        }
    }
    Err(TwitchAPIError::RequestError)
}
