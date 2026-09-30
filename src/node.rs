pub(crate) mod ether;
pub(crate) mod ip;
mod phy;

use std::collections::HashMap;
use std::fmt::Display;
use std::thread;

use self::phy::Phy;

pub(crate) struct Node {
    pub(crate) id: u32,
    pub(crate) phys: HashMap<phy::Id, Phy>,
}

impl Node {
    pub(crate) fn new(id: u32) -> Self {
        Node {
            id,
            phys: HashMap::new(),
        }
    }

    pub(crate) fn add_phy(&mut self, id: phy::Id) -> Option<&mut Phy> {
        if self.phys.contains_key(&id) {
            return None;
        }
        let phy = Phy::new(id, self.id);
        self.phys.insert(id, phy);
        Some(self.phys.get_mut(&id).unwrap())
    }

    pub(crate) fn run(self, f: impl FnOnce(Node) + Send + 'static) -> thread::JoinHandle<()> {
        thread::spawn(move || f(self))
    }
}

trait DbgInfo: Display {
    fn node_id(&self) -> u32;
    fn dbg_header(&self) -> String {
        format!("Node#{:02} {:8}:", self.node_id(), self.to_string())
    }
}
