use std::cmp::Ordering;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::Client;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct Level {
    pub size: f64,
    pub price: f64,
}

impl Level {
    /// Try to parse a Level from given value.
    ///
    /// For simplicity we are using Option here, however failing silently like this (without
    /// context) can make it harder to debug if/when the API breaks. Since we haven't built out
    /// much error reporting, this is fine for now.
    fn from_value(value: &Value) -> Option<Self> {
        if let [price, size] = value.as_array()?.as_slice() {
            Some(Self {
                size: size.as_str()?.parse().ok()?,
                price: price.as_str()?.parse().ok()?,
            })
        } else {
            None
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl TryFrom<&str> for OrderSide {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "buy" => Ok(OrderSide::Buy),
            "sell" => Ok(OrderSide::Sell),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct OrderBookDiff {
    pub order_side: OrderSide,
    pub level: Level,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Level2Update {
    #[serde(rename = "type")]
    type_: String,
    pub product_id: String,
    pub changes: Vec<OrderBookDiff>,
    pub time: DateTime<Utc>,
}

impl Level2Update {
    /// Try to parse a Level2Update from given value.
    ///
    /// For simplicity we are using Option here, however failing silently like this (without
    /// context) can make it harder to debug if/when the API breaks. Since we haven't built out
    /// much error reporting, this is fine for now.
    fn from_value(value: &Value) -> Option<Self> {
        let type_ = value.get("type")?.as_str()?.to_owned();
        let product_id = value.get("product_id")?.as_str()?.to_owned();
        let time = value.get("time")?.as_str()?.parse().ok()?;
        let changes = value
            .get("changes")?
            .as_array()?
            .iter()
            .filter_map(|arr| {
                if let [order_side, price, size] = arr.as_array()?.as_slice() {
                    let order_side = order_side.as_str()?.try_into().ok()?;
                    let level = Level {
                        size: size.as_str()?.parse().ok()?,
                        price: price.as_str()?.parse().ok()?,
                    };
                    Some(OrderBookDiff { order_side, level })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        Some(Self {
            type_,
            product_id,
            time,
            changes,
        })
    }
}

/// Order book state.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct OrderBook {
    #[serde(rename = "type")]
    type_: String,
    pub product_id: String,
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
}

impl OrderBook {
    /// Try to parse an OrderBook from given value.
    ///
    /// For simplicity we are using Option here, however failing silently like this (without
    /// context) can make it harder to debug if/when the API breaks. Since we haven't built out
    /// much error reporting, this is fine for now.
    fn from_value(value: &Value) -> Option<Self> {
        fn parse_levels(value: &Value) -> Option<Vec<Level>> {
            Some(
                value
                    .as_array()?
                    .iter()
                    .filter_map(Level::from_value)
                    .collect::<Vec<_>>(),
            )
        }

        let type_ = value.get("type")?.as_str()?.to_owned();
        let product_id = value.get("product_id")?.as_str()?.to_owned();
        let bids = parse_levels(value.get("bids")?)?;
        let asks = parse_levels(value.get("asks")?)?;

        Some(Self {
            type_,
            product_id,
            bids,
            asks,
        })
    }
}

pub async fn update_order_book(snapshot: &mut OrderBook, client: &mut Client) -> eyre::Result<()> {
    let frame = client.next_frame().await?;
    // If an l2update is received, update the order book.
    if let Some(l2update) = Level2Update::from_value(&frame) {
        for change in l2update.changes.into_iter() {
            // Ignore updates with 0 order size.
            if change.level.size == 0. {
                continue;
            }

            match change.order_side {
                OrderSide::Buy => {
                    snapshot.asks.push(change.level);
                    snapshot
                        .asks
                        .sort_by(|&a, &b| a.price.partial_cmp(&b.price).unwrap_or(Ordering::Less));
                }
                OrderSide::Sell => {
                    snapshot.bids.push(change.level);
                    snapshot
                        .bids
                        .sort_by(|&a, &b| a.price.partial_cmp(&b.price).unwrap_or(Ordering::Less));
                }
            }
        }
    // Otherwise, if a new snapshot is received, update the current one.
    } else if let Some(new_snapshot) = OrderBook::from_value(&frame) {
        *snapshot = new_snapshot;
    }

    Ok(())
}
