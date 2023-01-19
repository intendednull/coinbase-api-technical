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
    fn from_value(value: &Value) -> Option<Self> {
        let changes = value
            .get("changes")?
            .as_array()?
            .iter()
            .filter_map(|arr| {
                if let [order_side, price, size] =
                    arr.as_array().expect("unexpected type").as_slice()
                {
                    let order_side = order_side.as_str().unwrap().try_into().unwrap();
                    let level = Level {
                        size: size.as_str().unwrap().parse().unwrap(),
                        price: price.as_str().unwrap().parse().unwrap(),
                    };
                    Some(OrderBookDiff { order_side, level })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        Some(Self {
            type_: value.get("type")?.as_str()?.to_owned(),
            product_id: value.get("product_id")?.as_str()?.to_owned(),
            time: value.get("time")?.as_str()?.parse().ok()?,
            changes,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Snapshot {
    #[serde(rename = "type")]
    type_: String,
    pub product_id: String,
    // #[serde(deserialize_with = "parse_market_data")]
    pub bids: Vec<Level>,
    // #[serde(deserialize_with = "parse_market_data")]
    pub asks: Vec<Level>,
}

impl Snapshot {
    fn from_value(value: &Value) -> Option<Self> {
        let bids = value
            .get("bids")?
            .as_array()?
            .iter()
            .filter_map(|arr| {
                if let [price, size] = arr.as_array().unwrap().as_slice() {
                    Some(Level {
                        size: size.as_str().unwrap().parse().unwrap(),
                        price: price.as_str().unwrap().parse().unwrap(),
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let asks = value
            .get("asks")?
            .as_array()?
            .iter()
            .filter_map(|arr| {
                if let [price, size] = arr.as_array().unwrap().as_slice() {
                    Some(Level {
                        size: size.as_str().unwrap().parse().unwrap(),
                        price: price.as_str().unwrap().parse().unwrap(),
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        Some(Self {
            type_: value.get("type")?.as_str()?.to_owned(),
            product_id: value.get("product_id")?.as_str()?.to_owned(),
            bids,
            asks,
        })
    }
}

pub async fn update_order_book(snapshot: &mut Snapshot, client: &mut Client) -> eyre::Result<()> {
    let frame = client.next_frame().await?;
    // If an l2update is received, update the order book.
    if let Some(l2update) = Level2Update::from_value(&frame) {
        for change in l2update.changes.into_iter() {
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
    } else if let Some(new_snapshot) = Snapshot::from_value(&frame) {
        *snapshot = new_snapshot;
    }

    Ok(())
}
