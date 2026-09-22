mod node;

use node::ether::Ether;
use node::ip::Ip;
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
            let ip_addr = [192, 168, 77, node.id as u8];
            let phy = node.phys.remove(&0).unwrap();
            let ether = Ether::new(mac_addr, phy);
            let mut ip = Ip::new(ip_addr, ether);

            while let Some(data) = ip.recv() {
                println!("Node#{} received: {:02X?}", node.id, data);

                if let ip::Msg::Data(data) = data {
                    println!("Node#{} sending back", node.id);
                    ip.send([192, 168, 77, 1], &data);
                }
            }

            println!("Node#{} exiting.", node.id);
        }),
        node1.run(|mut node| {
            let mac_addr = [0x02, 0, 0, 0, 0, node.id as u8];
            let ip_addr = [192, 168, 77, node.id as u8];
            let phy = node.phys.remove(&0).unwrap();
            let ether = Ether::new(mac_addr, phy);
            let mut ip = Ip::new(ip_addr, ether);

            let payload = vec![0, 1, 2, 3];

            let mut sent = false;
            loop {
                if !sent {
                    println!("Node#{} sending: {:02X?}", node.id, payload);
                    sent = ip.send([192, 168, 77, 0], &payload);
                }

                if let Some(data) = ip.recv() {
                    println!("Node#{} received: {:02X?}", node.id, data);
                } else {
                    break;
                }
            }

            println!("Node#{} exiting.", node.id);
        }),
    ];

    // Wait for nodes to finish
    for handle in handles {
        handle.join().unwrap();
    }
}
