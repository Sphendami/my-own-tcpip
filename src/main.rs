mod node;

use crate::node::ether::{Ether, MacAddr};
use crate::node::ip::{Ip, IpAddr, IpIf, NextHop, Subnet};
use crate::node::*;

fn main() {
    // Create nodes
    let mut node0 = Node::new(0);
    let phy = node0.add_phy(0).unwrap();
    phy.add_port(0).unwrap();

    let mut node1 = Node::new(1);
    let phy = node1.add_phy(0).unwrap();
    phy.add_port(0).unwrap();
    let phy = node1.add_phy(1).unwrap();
    phy.add_port(0).unwrap();

    let mut node2 = Node::new(2);
    let phy = node2.add_phy(0).unwrap();
    phy.add_port(0).unwrap();

    // Connect ports
    let n0p0_0 = node0.get_port_mut(0, 0).unwrap();
    let n1p0_0 = node1.get_port_mut(0, 0).unwrap();
    n0p0_0.connect(n1p0_0).unwrap();
    let n1p1_0 = node1.get_port_mut(1, 0).unwrap();
    let n2p0_0 = node2.get_port_mut(0, 0).unwrap();
    n1p1_0.connect(n2p0_0).unwrap();

    let subnet77 = Subnet::new(IpAddr([192, 168, 77, 0]), IpAddr([255, 255, 255, 0]));
    let subnet78 = Subnet::new(IpAddr([192, 168, 78, 0]), IpAddr([255, 255, 255, 0]));
    let node0_ip = IpAddr([192, 168, 77, 0]);
    let node1_77_ip = IpAddr([192, 168, 77, 1]);
    let node1_78_ip = IpAddr([192, 168, 78, 1]);
    let node2_ip = IpAddr([192, 168, 78, 2]);

    // Run nodes
    let handles = [
        node0.run(move |mut node| {
            let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
            let phy = node.phys.remove(&0).unwrap();
            let ether = Ether::new(mac_addr, phy);
            let ipif = IpIf::new(node0_ip, ether);
            let mut ip = Ip::new(vec![ipif]);
            ip.add_routing(subnet77, NextHop::Direct, node0_ip);
            ip.add_routing(subnet78, NextHop::Indirect(node1_77_ip), node0_ip);

            loop {
                let ip::Msg(data) = ip.recv();
                println!("Node#{:02} received: {:02X?}", node.id, data);

                println!("Node#{:02} sending back", node.id);
                ip.send(Ip::IP_PROTOCOL_UDP, node2_ip, &data);
            }

            // println!("Node#{:02} exiting.", node.id);
        }),
        node1.run(move |mut node| {
            let ipif0 = {
                let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
                let phy = node.phys.remove(&0).unwrap();
                let ether = Ether::new(mac_addr, phy);
                IpIf::new(node1_77_ip, ether)
            };
            let ipif1 = {
                let mac_addr = MacAddr([0x02, 0, 0, 0, 1, node.id as u8]);
                let phy = node.phys.remove(&1).unwrap();
                let ether = Ether::new(mac_addr, phy);
                IpIf::new(node1_78_ip, ether)
            };
            let mut ip = Ip::new(vec![ipif0, ipif1]);
            ip.add_routing(subnet77, NextHop::Direct, node1_77_ip);
            ip.add_routing(subnet78, NextHop::Direct, node1_78_ip);

            loop {
                let ip::Msg(data) = ip.recv();
                println!("Node#{:02} received: {:02X?}", node.id, data);
            }

            // println!("Node#{:02} exiting.", node.id);
        }),
        node2.run(move |mut node| {
            let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
            let phy = node.phys.remove(&0).unwrap();
            let ether = Ether::new(mac_addr, phy);
            let ipif = IpIf::new(node2_ip, ether);
            let mut ip = Ip::new(vec![ipif]);
            ip.add_routing(subnet77, NextHop::Indirect(node1_78_ip), node2_ip);
            ip.add_routing(subnet78, NextHop::Direct, node2_ip);

            let payload = vec![0, 1, 2, 3];

            println!("Node#{:02} sending: {:02X?}", node.id, payload);
            ip.send(Ip::IP_PROTOCOL_UDP, node0_ip, &payload);
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
