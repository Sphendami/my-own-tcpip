use std::fmt::Display;
use std::sync::mpsc;

use crate::node::ether::{self, MacAddr};
use crate::node::{Common, DbgInfo, NextOp};

pub(crate) type PortId = u32;

enum PortState {
    NoMate {
        self_tx: mpsc::Sender<Vec<u8>>,
    },
    Mate {
        tx: mpsc::Sender<Vec<u8>>,
        name: String,
    },
}

pub(crate) struct SArg {
    pub(crate) data: Vec<u8>,
}

pub(crate) struct Port {
    id: PortId,
    rx: mpsc::Receiver<Vec<u8>>,
    state: PortState,
    upper: MacAddr,
    common: Common,
}

impl Port {
    pub(crate) fn new(id: PortId, upper: MacAddr, common: Common) -> Self {
        let (tx, rx) = mpsc::channel();
        Port {
            id,
            rx,
            state: PortState::NoMate { self_tx: tx },
            upper,
            common,
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
            tx: mate_tx,
            name: format!("Node#{:02} {}", mate.node_id(), mate),
        };
        mate.state = PortState::Mate {
            tx: self_tx,
            name: format!("Node#{:02} {}", self.node_id(), self),
        };
        Ok(())
    }

    pub(crate) fn try_recv(&self) -> bool {
        if let Ok(data) = self.rx.try_recv() {
            self.common.invoke(NextOp::EtherReceive {
                ether_id: self.upper,
                arg: ether::RArg {
                    port_id: self.id,
                    data,
                },
            });
            true
        } else {
            false
        }
    }

    pub(crate) fn send(&self, arg: SArg) {
        let SArg { data } = arg;

        match self.state {
            PortState::NoMate { .. } => {
                eprintln!("{} not connected (ignore the send)", self.dbg_header());
            }
            PortState::Mate { ref tx, ref name } => {
                println!("{} sending to {}: {:02x?}", self.dbg_header(), name, &data);
                tx.send(data)
                    .unwrap_or_else(|e| eprintln!("{} {} (ignore the send)", self.dbg_header(), e));
            }
        }
    }
}

impl Display for Port {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Port#{}", self.id)
    }
}

impl DbgInfo for Port {
    fn node_id(&self) -> u32 {
        self.common.node_id
    }
}
