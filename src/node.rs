mod ether;

use ether::Ether;
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;

type PortId = u32;

#[derive(Debug)]
enum Msg {
    Meta(Vec<u8>),
    Normal { port_id: PortId, data: Vec<u8> },
}

struct Port {
    #[allow(dead_code)]
    id: PortId,
    mate_id: PortId,
    mate_tx: mpsc::Sender<Msg>,
}

impl Port {
    fn send(&self, data: Vec<u8>) {
        self.mate_tx
            .send(Msg::Normal {
                port_id: self.mate_id,
                data,
            })
            .unwrap();
    }
}

pub(crate) struct MetaPort {
    #[allow(dead_code)]
    node_id: u32,
    tx: mpsc::Sender<Msg>,
}

impl MetaPort {
    pub(crate) fn send(&self, data: Vec<u8>) {
        self.tx.send(Msg::Meta(data)).unwrap();
    }
}

pub(crate) struct Node {
    id: u32,
    rx: mpsc::Receiver<Msg>,
    tx: mpsc::Sender<Msg>,
    ports: HashMap<PortId, Port>,
}

impl Node {
    pub(crate) fn new(id: u32) -> Self {
        let (tx, rx) = mpsc::channel();
        Node {
            id,
            rx,
            tx,
            ports: HashMap::new(),
        }
    }

    pub(crate) fn meta_port(&self) -> MetaPort {
        MetaPort {
            node_id: self.id,
            tx: self.tx.clone(),
        }
    }

    pub(crate) fn connect(&mut self, mate: &mut Node) {
        let self_port_id = self.ports.len() as PortId;
        let mate_port_id = mate.ports.len() as PortId;
        let self_port = Port {
            id: self_port_id,
            mate_id: mate_port_id,
            mate_tx: mate.tx.clone(),
        };
        let mate_port = Port {
            id: mate_port_id,
            mate_id: self_port_id,
            mate_tx: self.tx.clone(),
        };
        self.ports.insert(self_port_id, self_port);
        mate.ports.insert(mate_port_id, mate_port);

        println!(
            "Node#{}-Port#{} and Node#{}-Port#{} connected.",
            self.id, self_port_id, mate.id, mate_port_id
        );
    }

    pub(crate) fn run(self) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let mac_addr = [0x02, 0, 0, 0, 0, self.id as u8];
            let mut ether = Ether::new(mac_addr, &self.ports);
            while let Ok(message) = self.rx.recv() {
                println!("Node#{} received: {:02X?}", self.id, message);

                match message {
                    Msg::Meta(data) => {
                        ether.send(Ether::MAC_ADDR_BROADCAST, Ether::ETHERTYPE_LOCAL1, &data)
                    }
                    Msg::Normal { port_id, data } => {
                        ether.recv(port_id, data);
                    }
                }
            }

            println!("Node#{} exiting.", self.id);
        })
    }
}
