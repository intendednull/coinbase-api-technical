use eyre::bail;
use serde_json::{json, Value};
use websockets::{Frame, WebSocket};

/// Facilitates websocket connectin to coinbase.
pub struct Client {
    ws: WebSocket,
}

impl Client {
    /// Connect and subscribe to coinbase websocket feed using given crypto identifier.
    pub async fn subscribe(ident: &str) -> eyre::Result<Self> {
        let mut ws = WebSocket::connect("wss://ws-feed.exchange.coinbase.com").await?;

        ws.send_text(
            json! ({
                "type": "subscribe",
                "product_ids": [ident],
                "channels": ["level2"]
            })
            .to_string(),
        )
        .await?;

        Ok(Self { ws })
    }

    /// Wait for the next frame.
    pub async fn next_frame(&mut self) -> eyre::Result<Value> {
        let Frame::Text { payload, .. } = self.ws.receive().await? else {
            bail!("Error receiving next frame");
        };

        Ok(serde_json::from_str(&payload)?)
    }

    /// Disconnect this client.
    pub async fn close(&mut self) -> eyre::Result<()> {
        self.ws.close(None).await?;
        Ok(())
    }
}
