pub(crate) mod ether;
pub(crate) mod iface;
pub(crate) mod ip;
pub(crate) mod port;

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::mpsc;
use std::thread;

use self::ether::{Ether, MacAddr};
use self::iface::Iface;
use self::ip::{Ip, IpAddr};
use self::port::{Port, PortId};

pub(crate) enum NextOp {
    PortSend { port_id: PortId, arg: port::SArg },
    EtherSend { ether_id: MacAddr, arg: ether::SArg },
    IfaceSend { iface_id: IpAddr, arg: iface::SArg },
    IpSend { arg: ip::SArg },
    EtherReceive { ether_id: MacAddr, arg: ether::RArg },
    IfaceReceive { iface_id: IpAddr, arg: iface::RArg },
    IpReceive { arg: ip::RArg },
}

#[derive(Debug, Clone)]
pub(crate) struct Common {
    pub(crate) node_id: u32,
    op_tx: mpsc::Sender<NextOp>,
}

impl Common {
    pub(crate) fn invoke(&self, next_op: NextOp) {
        self.op_tx.send(next_op).unwrap();
    }
}

pub(crate) struct Node {
    pub(crate) id: u32,
    op_rx: mpsc::Receiver<NextOp>,
    pub(crate) common: Common,
    pub(crate) ip: Option<Ip>,
    pub(crate) ifaces: HashMap<IpAddr, Iface>,
    pub(crate) ethers: HashMap<MacAddr, Ether>,
    pub(crate) ports: HashMap<PortId, Port>,
    pub(crate) on_recv: Option<Box<dyn Fn(Common, Vec<u8>) + Send>>,
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
            ports: HashMap::new(),
            on_recv: None,
        }
    }

    pub(crate) fn run(mut self) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(1));

                // poll received data
                self.ports.values().for_each(|port| {
                    port.try_recv();
                });

                // handle next operation
                let Ok(op) = self.op_rx.try_recv() else {
                    continue;
                };
                match op {
                    NextOp::PortSend { port_id, arg } => {
                        if let Some(port) = self.ports.get(&port_id) {
                            port.send(arg);
                        } else {
                            eprintln!(
                                "Node#{:02} Port#{} not found (ignore the send)",
                                self.id, port_id
                            );
                        }
                    }
                    NextOp::EtherSend { ether_id, arg } => {
                        if let Some(ether) = self.ethers.get(&ether_id) {
                            ether.send(arg);
                        } else {
                            eprintln!(
                                "Node#{:02} Ether#{} not found (ignore the send)",
                                self.id, ether_id
                            );
                        }
                    }
                    NextOp::IfaceSend { iface_id, arg } => {
                        if let Some(iface) = self.ifaces.get_mut(&iface_id) {
                            iface.send(arg);
                        } else {
                            eprintln!(
                                "Node#{:02} Iface#{} not found (ignore the send)",
                                self.id, iface_id
                            );
                        }
                    }
                    NextOp::IpSend { arg } => {
                        if let Some(ip) = &mut self.ip {
                            ip.send(arg);
                        } else {
                            eprintln!("Node#{:02} Ip not found (ignore the send)", self.id);
                        }
                    }
                    NextOp::EtherReceive { ether_id, arg } => {
                        if let Some(ether) = self.ethers.get_mut(&ether_id) {
                            ether.recv(arg);
                        } else {
                            eprintln!(
                                "Node#{:02} Ether#{} not found (ignore the receive)",
                                self.id, ether_id
                            );
                        }
                    }
                    NextOp::IfaceReceive { iface_id, arg } => {
                        if let Some(iface) = self.ifaces.get_mut(&iface_id) {
                            iface.recv(arg);
                        } else {
                            eprintln!(
                                "Node#{:02} Iface#{} not found (ignore the receive)",
                                self.id, iface_id
                            );
                        }
                    }
                    NextOp::IpReceive { arg } => {
                        if let Some(ip) = &self.ip {
                            let data = ip.recv(arg);
                            if let Some(data) = data
                                && let Some(on_recv) = &self.on_recv
                            {
                                on_recv(self.common.clone(), data);
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
