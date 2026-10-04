mod node;

use crate::node::ether::{Ether, MacAddr};
use crate::node::ip::{Ip, IpAddr, IpIf, Subnet};
use crate::node::*;

fn main() {
    // Create nodes
    let mut node0 = Node::new(0);
    let phy = node0.add_phy(0).unwrap();
    let node0_phy0_port0 = phy.add_port(0).unwrap();

    let mut node1 = Node::new(1);
    let phy = node1.add_phy(0).unwrap();
    let node1_phy0_port0 = phy.add_port(0).unwrap();

    // Connect ports
    node0_phy0_port0.connect(node1_phy0_port0).unwrap();

    // Run nodes
    let handles = [
        node0.run(|mut node| {
            let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
            let ip_addr = IpAddr([192, 168, 77, node.id as u8]);
            let subnet = Subnet::new(ip_addr, IpAddr([255, 255, 255, 0]));
            let phy = node.phys.remove(&0).unwrap();
            let ether = Ether::new(mac_addr, phy);
            let ipif = IpIf::new(ip_addr, ether);
            let mut ip = Ip::new(vec![ipif]);
            ip.add_routing(subnet, ip_addr);

            loop {
                let ip::Msg(data) = ip.recv();
                println!("Node#{:02} received: {:02X?}", node.id, data);

                println!("Node#{:02} sending back", node.id);
                ip.send(Ip::IP_PROTOCOL_UDP, IpAddr([192, 168, 77, 1]), &data);
            }

            // println!("Node#{:02} exiting.", node.id);
        }),
        node1.run(|mut node| {
            let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
            let ip_addr = IpAddr([192, 168, 77, node.id as u8]);
            let subnet = Subnet::new(ip_addr, IpAddr([255, 255, 255, 0]));
            let phy = node.phys.remove(&0).unwrap();
            let ether = Ether::new(mac_addr, phy);
            let ipif = IpIf::new(ip_addr, ether);
            let mut ip = Ip::new(vec![ipif]);
            ip.add_routing(subnet, ip_addr);

            let payload = vec![0, 1, 2, 3];

            println!("Node#{:02} sending: {:02X?}", node.id, payload);
            ip.send(Ip::IP_PROTOCOL_UDP, IpAddr([192, 168, 77, 0]), &payload);
            loop {
                let ip::Msg(data) = ip.recv();
                println!("Node#{:02} received: {:02X?}", node.id, data);
            }

            // println!("Node#{:02} exiting.", node.id);
        }),
    ];

    // Wait for nodes to finish
    for handle in handles {
        handle.join().unwrap();
    }
}
