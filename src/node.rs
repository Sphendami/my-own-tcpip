pub(crate) mod ether;
pub(crate) mod iface;
pub(crate) mod ip;
pub(crate) mod phy;

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::mpsc;
use std::thread;

use self::ether::{Ether, MacAddr};
use self::iface::Iface;
use self::ip::{Ip, IpAddr};
use self::phy::Phy;

pub(crate) enum NextOp {
    PhySend {
        phy_id: phy::Id,
        port_id: phy::PortId,
        data: Vec<u8>,
    },
    EtherSend {
        ether_id: MacAddr,
        dst_mac: MacAddr,
        ethertype: u16,
        payload: Vec<u8>,
    },
    IfaceSend {
        iface_id: IpAddr,
        next_ip: IpAddr,
        data: Vec<u8>,
    },
    IpSend {
        protocol: u8,
        dst_ip: IpAddr,
        payload: Vec<u8>,
    },
    EtherReceive {
        ether_id: MacAddr,
        msg: phy::Msg,
    },
    IfaceReceive {
        iface_id: IpAddr,
        msg: ether::Msg,
    },
    IpReceive {
        msg: iface::Msg,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct Common {
    pub(crate) node_id: u32,
    pub(crate) op_tx: mpsc::Sender<NextOp>,
}

pub(crate) struct Node {
    pub(crate) id: u32,
    op_rx: mpsc::Receiver<NextOp>,
    pub(crate) common: Common,
    pub(crate) ip: Option<Ip>,
    pub(crate) ifaces: HashMap<IpAddr, Iface>,
    pub(crate) ethers: HashMap<MacAddr, Ether>,
    pub(crate) phys: HashMap<phy::Id, Phy>,
    pub(crate) on_recv: Option<Box<dyn Fn(Common, ip::Msg) + Send>>,
}

impl Node {
    pub(crate) fn new(id: u32) -> Self {
        let (op_tx, op_rx) = mpsc::channel();
        Node {
            id,
            op_rx,
            common: Common { node_id: id, op_tx },
            ip: None,
            ifaces: HashMap::new(),
            ethers: HashMap::new(),
            phys: HashMap::new(),
            on_recv: None,
        }
    }

    pub(crate) fn get_port_mut(
        &mut self,
        phy_id: phy::Id,
        port_id: phy::PortId,
    ) -> Option<&mut phy::Port> {
        self.phys.get_mut(&phy_id)?.ports.get_mut(&port_id)
    }

    pub(crate) fn run(mut self) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(1));

                // poll received data
                self.phys.values().for_each(|phy| {
                    phy.try_recv();
                });

                // handle next operation
                let Ok(op) = self.op_rx.try_recv() else {
                    continue;
                };
                match op {
                    NextOp::PhySend {
                        phy_id,
                        port_id,
                        data,
                    } => {
                        if let Some(phy) = self.phys.get(&phy_id) {
                            phy.send(port_id, data);
                        } else {
                            eprintln!(
                                "Node#{:02} Phy#{} not found (ignore the send)",
                                self.id, phy_id
                            );
                        }
                    }
                    NextOp::EtherSend {
                        ether_id,
                        dst_mac,
                        ethertype,
                        payload,
                    } => {
                        if let Some(ether) = self.ethers.get(&ether_id) {
                            ether.send(dst_mac, ethertype, &payload);
                        } else {
                            eprintln!(
                                "Node#{:02} Ether#{} not found (ignore the send)",
                                self.id, ether_id
                            );
                        }
                    }
                    NextOp::IfaceSend {
                        iface_id,
                        next_ip,
                        data,
                    } => {
                        if let Some(iface) = self.ifaces.get_mut(&iface_id) {
                            iface.send(next_ip, &data);
                        } else {
                            eprintln!(
                                "Node#{:02} Iface#{} not found (ignore the send)",
                                self.id, iface_id
                            );
                        }
                    }
                    NextOp::IpSend {
                        protocol,
                        dst_ip,
                        payload,
                    } => {
                        if let Some(ip) = &mut self.ip {
                            ip.send(protocol, dst_ip, &payload);
                        } else {
                            eprintln!("Node#{:02} Ip not found (ignore the send)", self.id);
                        }
                    }
                    NextOp::EtherReceive { ether_id, msg } => {
                        if let Some(ether) = self.ethers.get_mut(&ether_id) {
                            ether.recv(msg);
                        } else {
                            eprintln!(
                                "Node#{:02} Ether#{} not found (ignore the receive)",
                                self.id, ether_id
                            );
                        }
                    }
                    NextOp::IfaceReceive { iface_id, msg } => {
                        if let Some(iface) = self.ifaces.get_mut(&iface_id) {
                            iface.recv(msg);
                        } else {
                            eprintln!(
                                "Node#{:02} Iface#{} not found (ignore the receive)",
                                self.id, iface_id
                            );
                        }
                    }
                    NextOp::IpReceive { msg } => {
                        if let Some(ip) = &self.ip {
                            let msg = ip.recv(msg);
                            if let Some(msg) = msg
                                && let Some(on_recv) = &self.on_recv
                            {
                                on_recv(self.common.clone(), msg);
                            }
                        } else {
                            eprintln!("Node#{:02} Ip not found (ignore the receive)", self.id);
                        }
                    }
                }
            }
        })
    }
}

trait DbgInfo: Display {
    fn node_id(&self) -> u32;
    fn dbg_header(&self) -> String {
        format!("Node#{:02} {:8}:", self.node_id(), self.to_string())
    }
}
