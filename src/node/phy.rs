use std::collections::HashMap;
use std::fmt::Display;
use std::sync::mpsc;

use crate::node::DbgInfo;

pub(crate) type PortId = u32;

struct PortDbgInfo {
    node_id: u32,
    phy_id: Id,
}

#[derive(Debug)]
pub(crate) struct Msg {
    pub(crate) port_id: PortId,
    pub(crate) data: Vec<u8>,
}

enum PortState {
    NoMate {
        self_tx: mpsc::Sender<Msg>,
    },
    Mate {
        port_id: PortId,
        tx: mpsc::Sender<Msg>,
        name: String,
    },
}

pub(crate) struct Port {
    id: PortId,
    state: PortState,
    dbg: PortDbgInfo,
}

impl Port {
    pub(crate) fn new(id: PortId, self_tx: mpsc::Sender<Msg>, node_id: u32, phy_id: Id) -> Self {
        Port {
            id,
            state: PortState::NoMate { self_tx },
            dbg: PortDbgInfo { node_id, phy_id },
        }
    }

    pub(crate) fn connect(&mut self, mate: &mut Self) -> Result<(), String> {
        // there are unnecessary clones...
        let mate_tx = match &mate.state {
            PortState::NoMate { self_tx } => self_tx.clone(),
            PortState::Mate { name, .. } => {
                return Err(format!(
                    "{} already connected to {}",
                    mate.dbg_header(),
                    name
                ));
            }
        };

        let self_tx = match &self.state {
            PortState::NoMate { self_tx } => self_tx.clone(),
            PortState::Mate { name, .. } => {
                return Err(format!(
                    "{} already connected to {}",
                    self.dbg_header(),
                    name
                ));
            }
        };

        self.state = PortState::Mate {
            port_id: mate.id,
            tx: mate_tx,
            name: format!("Node#{:02} {}", mate.node_id(), mate),
        };
        mate.state = PortState::Mate {
            port_id: self.id,
            tx: self_tx,
            name: format!("Node#{:02} {}", self.node_id(), self),
        };
        Ok(())
    }

    pub(crate) fn send(&self, data: Vec<u8>) -> Result<(), String> {
        match self.state {
            PortState::NoMate { .. } => {
                return Err(format!("{} not connected", self.dbg_header()));
            }
            PortState::Mate {
                port_id,
                ref tx,
                ref name,
            } => {
                println!("{} sending to {}: {:02x?}", self.dbg_header(), name, &data);
                tx.send(Msg { port_id, data })
                    .map_err(|e| format!("{} {}", self.dbg_header(), e))
            }
        }
    }
}

impl Display for Port {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Port#{}-{}", self.dbg.phy_id, self.id)
    }
}

impl DbgInfo for Port {
    fn node_id(&self) -> u32 {
        self.dbg.node_id
    }
}

pub(crate) type Id = u32;

struct PhyDbgInfo {
    node_id: u32,
}

/** Collection of ports within the same local network */
pub(crate) struct Phy {
    #[allow(dead_code)]
    id: Id,
    rx: mpsc::Receiver<Msg>,
    tx: mpsc::Sender<Msg>,
    pub(crate) ports: HashMap<PortId, Port>,
    dbg: PhyDbgInfo,
}

impl Phy {
    pub(crate) fn new(id: Id, node_id: u32) -> Self {
        let (tx, rx) = mpsc::channel();
        Phy {
            id,
            rx,
            tx,
            ports: HashMap::new(),
            dbg: PhyDbgInfo { node_id },
        }
    }

    pub(crate) fn add_port(&mut self, id: PortId) -> Option<&mut Port> {
        if self.ports.contains_key(&id) {
            return None;
        }
        let port = Port::new(id, self.tx.clone(), self.dbg.node_id, self.id);
        self.ports.insert(id, port);
        Some(self.ports.get_mut(&id).unwrap())
    }

    pub(crate) fn port_ids(&self) -> Vec<PortId> {
        self.ports.keys().copied().collect()
    }

    pub(crate) fn try_recv(&self) -> Option<Msg> {
        self.rx.try_recv().ok()
    }

    pub(crate) fn recv(&self) -> Msg {
        self.rx.recv().unwrap()
    }

    pub(crate) fn send(&self, port_id: PortId, data: Vec<u8>) {
        if let Some(port) = self.ports.get(&port_id) {
            port.send(data)
                .unwrap_or_else(|e| eprintln!("{} (ignore the send)", e));
        } else {
            eprintln!(
                "{} Port#{} not found (ignore the send)",
                self.dbg_header(),
                port_id
            );
        }
    }
}

impl Display for Phy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Phy#{}", self.id)
    }
}

impl DbgInfo for Phy {
    fn node_id(&self) -> u32 {
        self.dbg.node_id
    }
}
