mod ether;

use ether::Ether;
use std::sync::mpsc;
use std::thread;

type Msg = Vec<u8>;

pub(crate) struct Node {
    id: u32,
    rx: mpsc::Receiver<Msg>,
    tx: mpsc::Sender<Msg>,
    ports: Vec<mpsc::Sender<Msg>>,
}

impl Node {
    pub(crate) fn new(id: u32) -> Self {
        let (tx, rx) = mpsc::channel();
        Node {
            id,
            rx,
            tx,
            ports: Vec::new(),
        }
    }

    pub(crate) fn tx(&self) -> mpsc::Sender<Msg> {
        self.tx.clone()
    }

    pub(crate) fn connect(&mut self, mate: &mut Node) {
        self.ports.push(mate.tx.clone());
        mate.ports.push(self.tx.clone());

        println!("Node {} and Node {} were connected.", self.id, mate.id);
    }

    pub(crate) fn run(self) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let mac_addr = [0x02, 0, 0, 0, 0, self.id as u8];
            let mut ether = Ether::new(mac_addr, &self.ports);
            while let Ok(message) = self.rx.recv() {
                println!("Node {} received: {:02X?}", self.id, message);

                if message[0] == 0 {
                    ether.send(
                        Ether::MAC_ADDR_BROADCAST,
                        Ether::ETHERTYPE_LOCAL1,
                        &message[1..],
                    );
                } else {
                    ether.recv(0, message); // todo
                }
            }

            println!("Node {} exiting.", self.id);
        })
    }
}
