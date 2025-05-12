use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value as SerdeValue;
use std::{collections::HashMap, sync::Arc};
use tauri::async_runtime::RwLock;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::{Filter, Reply, filters::BoxedFilter, ws::{Message, WebSocket, Ws}};

#[derive(Deserialize)]
pub struct PeerQueryData {
    id: String,
}

pub type Peers = Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Result<Message, warp::Error>>>>>;

pub fn path() -> BoxedFilter<(impl Reply,)> {
    let peers = Peers::default();
    let peers = warp::any().map(move || peers.clone());

    warp::path("peer")
        .and(warp::ws())
        .and(peers)
        .and(warp::query::<PeerQueryData>())
        .map(|ws: Ws, peers, q| ws.on_upgrade(move |socket| peer_handler(socket, peers, q)))
        .boxed()
}

pub async fn peer_handler(ws: WebSocket, peers: Peers, query: PeerQueryData) {
    let (peer_tx, mut peer_rx) = ws.split();

    let (tx, rx) = mpsc::unbounded_channel();
    let rx = UnboundedReceiverStream::new(rx);
    tauri::async_runtime::spawn(rx.forward(peer_tx));

    if peers.read().await.contains_key(&query.id) {
        println!("already registered");
        return;
    }

    tx.send(Ok(PeerMessageType::Open.into())).unwrap();

    peers.write().await.insert(query.id.clone(), tx);

    while let Some(result) = peer_rx.next().await {
        let Ok(msg) = result else {
            break;
        };
        handle_message(&query.id, msg, &peers).await;
    }
    peers.write().await.remove(&query.id);
}

#[derive(Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
enum PeerMessageType {
    Open, // socket ready
    Leave,
    Candidate,
    Offer,
    Answer,
    Expire, // host not found
    Heartbeat,
    #[serde(rename(serialize = "ID_TAKEN", deserialize = "IDTAKEN"))]
    IDTaken,
    #[default]
    Error,
}

impl From<PeerMessageType> for Message {
    fn from(val: PeerMessageType) -> Self {
        Message::text(serde_json::to_string(&PeerMessageShort { t: val }).unwrap())
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
struct PeerMessage {
    #[serde(rename(serialize = "type", deserialize = "type"))]
    #[serde(default)]
    pub t: PeerMessageType,
    #[serde(default)]
    pub src: String,
    #[serde(default)]
    pub dst: String,
    #[serde(default)]
    pub payload: SerdeValue,
}

#[derive(Serialize, Deserialize, PartialEq)]
struct PeerMessageShort {
    #[serde(rename(serialize = "type", deserialize = "type"))]
    #[serde(default)]
    pub t: PeerMessageType,
}

async fn handle_message(peer_id: &String, msg: Message, users: &Peers) {
    let Ok(msg_str) = msg.to_str() else { return };
    let Ok(mut msg) = serde_json::from_str::<PeerMessage>(msg_str) else {
        return;
    };

    let users = users.read().await;

    if msg.t == PeerMessageType::Offer && !users.contains_key(msg.dst.as_str()) {
        let Some(peer_tx) = users.get(peer_id) else { return };
        let Ok(msg_str) = serde_json::to_string(&PeerMessage {
            t: PeerMessageType::Expire,
            src: msg.dst,
            dst: peer_id.clone(),
            payload: msg.payload,
        }) else {
            return;
        };
        peer_tx.send(Ok(Message::text(msg_str))).unwrap();
    } else if !msg.dst.is_empty() && users.contains_key(&msg.dst) {
        msg.src = peer_id.clone();
        let Some(peer_tx) = users.get(&msg.dst) else { return };
        let Ok(msg_str) = serde_json::to_string(&msg) else { return };
        peer_tx.send(Ok(Message::text(msg_str))).unwrap();
    }
}
