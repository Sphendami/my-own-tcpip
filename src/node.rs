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
            while let Ok(message) = self.rx.recv() {
                println!("Node {} received: {:?}", self.id, message);

                if message[0] < 10 {
                    let new_message = vec![message[0] + 1];

                    println!("Node {} sent on Port {}: {:?}", self.id, 0, new_message);
                    self.ports[0].send(new_message).unwrap();
                }
            }

            println!("Node {} exiting.", self.id);
        })
    }
}
