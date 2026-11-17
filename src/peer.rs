use crate::common::*;
use crate::database;
use hbb_common::{
    bytes::Bytes,
    log,
    rendezvous_proto::*,
    tokio::sync::{Mutex, RwLock},
    ResultType,
};
use serde_derive::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    collections::HashSet,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Instant,
};

type IpBlockMap = HashMap<String, ((u32, Instant), (HashSet<String>, Instant))>;
type UserStatusMap = HashMap<Vec<u8>, Arc<(Option<Vec<u8>>, bool)>>;
type IpChangesMap = HashMap<String, (Instant, HashMap<String, i32>)>;
lazy_static::lazy_static! {
    pub(crate) static ref IP_BLOCKER: Mutex<IpBlockMap> = Default::default();
    pub(crate) static ref USER_STATUS: RwLock<UserStatusMap> = Default::default();
    pub(crate) static ref IP_CHANGES: Mutex<IpChangesMap> = Default::default();
}
pub const IP_CHANGE_DUR: u64 = 180;
pub const IP_CHANGE_DUR_X2: u64 = IP_CHANGE_DUR * 2;
pub const DAY_SECONDS: u64 = 3600 * 24;
pub const IP_BLOCK_DUR: u64 = 60;
// Same bound as the Pro server: a longer id is not a normal RustDesk id or hostname,
// and every peer entry and punch-request record keeps its own copy.
pub const MAX_PEER_ID_LEN: usize = 253;

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub(crate) struct PeerInfo {
    #[serde(default)]
    pub(crate) ip: String,
}

pub(crate) struct Peer {
    pub(crate) socket_addr: SocketAddr,
    pub(crate) last_reg_time: Instant,
    pub(crate) guid: Vec<u8>,
    pub(crate) uuid: Bytes,
    pub(crate) pk: Bytes,
    // pub(crate) user: Option<Vec<u8>>,
    pub(crate) info: PeerInfo,
    // pub(crate) disabled: bool,
    pub(crate) reg_pk: (u32, Instant), // how often register_pk
}

impl Default for Peer {
    fn default() -> Self {
        Self {
            socket_addr: "0.0.0.0:0".parse().unwrap(),
            last_reg_time: get_expired_time(),
            guid: Vec::new(),
            uuid: Bytes::new(),
            pk: Bytes::new(),
            info: Default::default(),
            // user: None,
            // disabled: false,
            reg_pk: (0, get_expired_time()),
        }
    }
}

pub(crate) type LockPeer = Arc<RwLock<Peer>>;

#[derive(Clone)]
pub(crate) struct PeerMap {
    map: Arc<RwLock<HashMap<String, LockPeer>>>,
    pub(crate) db: database::Database,
}

impl PeerMap {
    pub(crate) async fn new() -> ResultType<Self> {
        let db = get_arg_opt("DB_URL").unwrap_or_else(|| {
            let mut db = "db_v2.sqlite3".to_owned();
            #[cfg(all(windows, not(debug_assertions)))]
            {
                if let Some(path) = hbb_common::config::Config::icon_path().parent() {
                    db = format!("{}\\{}", path.to_str().unwrap_or("."), db);
                }
            }
            #[cfg(not(windows))]
            {
                db = format!("./{db}");
            }
            db
        });
        log::info!("DB_URL={}", db);
        let pm = Self {
            map: Default::default(),
            db: database::Database::new(&db).await?,
        };
        Ok(pm)
    }

    #[inline]
    pub(crate) async fn update_pk(
        &mut self,
        id: String,
        peer: LockPeer,
        addr: SocketAddr,
        uuid: Bytes,
        pk: Bytes,
        ip: String,
    ) -> register_pk_response::Result {
        log::info!("update_pk {} {:?} {:?} {:?}", id, addr, uuid, pk);
        let (info_str, guid) = {
            let mut w = peer.write().await;
            w.socket_addr = addr;
            w.uuid = uuid.clone();
            w.pk = pk.clone();
            w.last_reg_time = Instant::now();
            w.info.ip = ip;
            (
                serde_json::to_string(&w.info).unwrap_or_default(),
                w.guid.clone(),
            )
        };
        if guid.is_empty() {
            match self.db.insert_peer(&id, &uuid, &pk, &info_str).await {
                Err(err) => {
                    log::error!("db.insert_peer failed: {}", err);
                    return register_pk_response::Result::SERVER_ERROR;
                }
                Ok(guid) => {
                    peer.write().await.guid = guid;
                }
            }
        } else {
            if let Err(err) = self.db.update_pk(&guid, &pk, &info_str).await {
                log::error!("db.update_pk failed: {}", err);
                return register_pk_response::Result::SERVER_ERROR;
            }
            log::info!("pk updated instead of insert");
        }
        register_pk_response::Result::OK
    }

    pub(crate) async fn change_id(
        &self,
        old_id: &str,
        id: &str,
        uuid: &[u8],
        ip: IpAddr,
    ) -> register_pk_response::Result {
        let peer = match self.get(old_id).await {
            Some(peer) => peer,
            None => return register_pk_response::Result::UUID_MISMATCH,
        };
        let guid = {
            let peer = peer.read().await;
            // The uuid is not a secret (it travels in clear over UDP), so the request
            // must also come from the address the peer last registered from.
            let same_ip = peer
                .info
                .ip
                .parse::<IpAddr>()
                .map_or(false, |x| x.to_canonical() == ip.to_canonical());
            if peer.guid.is_empty()
                || peer.uuid.is_empty()
                || peer.uuid[..] != *uuid
                || !same_ip
            {
                return register_pk_response::Result::UUID_MISMATCH;
            }
            peer.guid.clone()
        };
        // Loads the row of the new id into the map, if there is one.
        self.get(id).await;
        // The map stays locked from the checks to the key move, so that two renames
        // of the same peer cannot both succeed.
        let mut map = self.map.write().await;
        if !map.get(old_id).map_or(false, |p| Arc::ptr_eq(p, &peer)) {
            return register_pk_response::Result::UUID_MISMATCH;
        }
        // get_or() leaves an entry with no uuid for an id that never finished registering.
        if let Some(other) = map.get(id) {
            if !other.read().await.uuid.is_empty() {
                return register_pk_response::Result::ID_EXISTS;
            }
        }
        if let Err(err) = self.db.update_id(&guid, id).await {
            log::error!("db.update_id failed: {}", err);
            return register_pk_response::Result::SERVER_ERROR;
        }
        map.remove(old_id);
        map.insert(id.to_owned(), peer);
        register_pk_response::Result::OK
    }

    #[inline]
    pub(crate) async fn get(&self, id: &str) -> Option<LockPeer> {
        let p = self.map.read().await.get(id).cloned();
        if p.is_some() {
            return p;
        } else if let Ok(Some(v)) = self.db.get_peer(id).await {
            let peer = Peer {
                guid: v.guid,
                uuid: v.uuid.into(),
                pk: v.pk.into(),
                // user: v.user,
                info: serde_json::from_str::<PeerInfo>(&v.info).unwrap_or_default(),
                // disabled: v.status == Some(0),
                ..Default::default()
            };
            let peer = Arc::new(RwLock::new(peer));
            self.map.write().await.insert(id.to_owned(), peer.clone());
            return Some(peer);
        }
        None
    }

    #[inline]
    pub(crate) async fn get_or(&self, id: &str) -> LockPeer {
        if let Some(p) = self.get(id).await {
            return p;
        }
        let mut w = self.map.write().await;
        if let Some(p) = w.get(id) {
            return p.clone();
        }
        let tmp = LockPeer::default();
        w.insert(id.to_owned(), tmp.clone());
        tmp
    }

    #[inline]
    pub(crate) async fn get_in_memory(&self, id: &str) -> Option<LockPeer> {
        self.map.read().await.get(id).cloned()
    }

    #[inline]
    pub(crate) async fn is_in_memory(&self, id: &str) -> bool {
        self.map.read().await.contains_key(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::tokio;
    use register_pk_response::Result::{ID_EXISTS, OK, UUID_MISMATCH};

    async fn peer_map(db: &std::path::Path) -> PeerMap {
        PeerMap {
            map: Default::default(),
            db: database::Database::new(db.to_str().unwrap()).await.unwrap(),
        }
    }

    #[tokio::test]
    async fn change_id_renames_only_for_the_owner_and_to_a_free_id() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("db.sqlite3");
        let mut pm = peer_map(&db).await;
        let addr = "127.0.0.1:1".parse().unwrap();
        for (id, uuid, ip) in [("111111", "u1", "1.1.1.1"), ("222222", "u2", "2.2.2.2")] {
            let peer = pm.get_or(id).await;
            let res = pm
                .update_pk(id.to_owned(), peer, addr, uuid.into(), "pk".into(), ip.to_owned())
                .await;
            assert_eq!(res, OK);
        }
        let ip1: IpAddr = "1.1.1.1".parse().unwrap();
        let ip2: IpAddr = "2.2.2.2".parse().unwrap();
        let ip1_mapped: IpAddr = "::ffff:1.1.1.1".parse().unwrap();
        let registering = pm.get("111111").await.unwrap();

        assert_eq!(pm.change_id("111111", "office-pc", b"u2", ip1).await, UUID_MISMATCH);
        assert_eq!(pm.change_id("111111", "office-pc", b"u1", ip2).await, UUID_MISMATCH);
        assert_eq!(pm.change_id("333333", "office-pc", b"u1", ip1).await, UUID_MISMATCH);
        assert_eq!(pm.change_id("111111", "222222", b"u1", ip1).await, ID_EXISTS);
        // An id that was only looked up, never registered, is still free.
        pm.get_or("office-pc").await;
        assert_eq!(pm.change_id("111111", "office-pc", b"u1", ip1_mapped).await, OK);
        assert_eq!(pm.change_id("111111", "other-pc", b"u1", ip1).await, UUID_MISMATCH);
        assert!(pm.get("111111").await.is_none());
        assert_eq!(pm.get("office-pc").await.unwrap().read().await.uuid, "u1");

        // A registration that was already in flight for the old id must not undo the rename.
        let res = pm
            .update_pk(
                "111111".to_owned(),
                registering,
                addr,
                "u1".into(),
                "pk2".into(),
                "1.1.1.1".to_owned(),
            )
            .await;
        assert_eq!(res, OK);

        let reopened = peer_map(&db).await;
        assert!(reopened.get("111111").await.is_none());
        assert_eq!(reopened.get("office-pc").await.unwrap().read().await.uuid, "u1");
    }
}
