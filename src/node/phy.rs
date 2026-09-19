use std::collections::HashMap;
use std::sync::mpsc;

pub(crate) type PortId = u32;

#[derive(Debug)]
pub(crate) struct Msg {
    pub(crate) port_id: PortId,
    pub(crate) data: Vec<u8>,
}

enum PortState {
    NoMate { self_tx: mpsc::Sender<Msg> },
    Mate(PortId, mpsc::Sender<Msg>),
}

pub(crate) struct Port {
    id: PortId,
    state: PortState,
}

impl Port {
    pub(crate) fn new(id: PortId, self_tx: mpsc::Sender<Msg>) -> Self {
        Port {
            id,
            state: PortState::NoMate { self_tx },
        }
    }

    pub(crate) fn connect(&mut self, mate: &mut Self) -> Option<()> {
        // todo: simplify
        let mate_tx = match &mate.state {
            PortState::NoMate { self_tx } => self_tx.clone(),
            PortState::Mate(_, _) => {
                eprintln!("Port#{} is already connected", mate.id);
                return None;
            }
        };

        let self_tx = match &self.state {
            PortState::NoMate { self_tx } => self_tx.clone(),
            PortState::Mate(_, _) => {
                eprintln!("Port#{} is already connected", self.id);
                return None;
            }
        };

        self.state = PortState::Mate(mate.id, mate_tx);
        mate.state = PortState::Mate(self.id, self_tx);
        Some(())
    }

    pub(crate) fn send(&self, data: Vec<u8>) {
        match self.state {
            PortState::NoMate { .. } => {
                eprintln!("Port#{} is not connected", self.id);
            }
            PortState::Mate(mate_port_id, ref mate_tx) => {
                mate_tx
                    .send(Msg {
                        port_id: mate_port_id,
                        data,
                    })
                    .unwrap();
            }
        }
    }
}

pub(crate) type Id = u32;

/** Collection of ports within the same local network */
pub(crate) struct Phy {
    #[allow(dead_code)]
    id: Id,
    rx: mpsc::Receiver<Msg>,
    tx: mpsc::Sender<Msg>,
    pub(crate) ports: HashMap<PortId, Port>,
}

impl Phy {
    pub(crate) fn new(id: Id) -> Self {
        let (tx, rx) = mpsc::channel();
        Phy {
            id,
            rx,
            tx,
            ports: HashMap::new(),
        }
    }

    pub(crate) fn add_port(&mut self, id: PortId) -> Option<&mut Port> {
        if self.ports.contains_key(&id) {
            return None;
        }
        let port = Port::new(id, self.tx.clone());
        self.ports.insert(id, port);
        Some(self.ports.get_mut(&id).unwrap())
    }

    pub(crate) fn recv(&self) -> Msg {
        self.rx.recv().unwrap()
    }

    pub(crate) fn send(&self, port_id: PortId, data: Vec<u8>) {
        if let Some(port) = self.ports.get(&port_id) {
            port.send(data);
        } else {
            eprintln!("Port#{} not found", port_id);
        }
    }

    pub(crate) fn send_to_all(&self, data: Vec<u8>) {
        for port in self.ports.values() {
            port.send(data.clone());
        }
    }
}
