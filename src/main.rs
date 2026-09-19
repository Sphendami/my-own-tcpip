mod node;

use node::ether::Ether;
use node::*;

fn main() {
    // Create nodes
    let mut node0 = Node::new(0);
    let phy = node0.add_phy(0).unwrap();
    let node0_phy0_port0 = phy.add_port(0).unwrap();

    let mut node1 = Node::new(1);
    let phy = node1.add_phy(0).unwrap();
    let node1_phy0_port0 = phy.add_port(0).unwrap();

    // Connect ports
    node0_phy0_port0.connect(node1_phy0_port0);

    // Run nodes
    let handles = [
        node0.run(|mut node| {
            let mac_addr = [0x02, 0, 0, 0, 0, node.id as u8];
            let phy = node.phys.remove(&0).unwrap();
            let mut ether = Ether::new(mac_addr, phy);

            while let Some(data) = ether.recv() {
                println!("Node#{} received: {:02X?}", node.id, data);

                ether.send(Ether::MAC_ADDR_BROADCAST, Ether::ETHERTYPE_LOCAL1, &data);
            }

            println!("Node#{} exiting.", node.id);
        }),
        node1.run(|mut node| {
            let mac_addr = [0x02, 0, 0, 0, 0, node.id as u8];
            let phy = node.phys.remove(&0).unwrap();
            let mut ether = Ether::new(mac_addr, phy);

            let payload = vec![0, 1, 2, 3];
            println!("Node#{} sending: {:02X?}", node.id, payload);
            ether.send(Ether::MAC_ADDR_BROADCAST, Ether::ETHERTYPE_LOCAL1, &payload);

            while let Some(data) = ether.recv() {
                println!("Node#{} received: {:02X?}", node.id, data);
            }

            println!("Node#{} exiting.", node.id);
        }),
    ];

    // Wait for nodes to finish
    for handle in handles {
        handle.join().unwrap();
    }
}
