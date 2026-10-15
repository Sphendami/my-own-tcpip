mod node;

use crate::node::ether::{Ether, MacAddr};
use crate::node::ip::{Ip, IpAddr, IpIf, NextHop, Subnet};
use crate::node::phy::Phy;
use crate::node::*;

fn main() {
    let subnet77 = Subnet::new(IpAddr([192, 168, 77, 0]), IpAddr([255, 255, 255, 0]));
    let subnet78 = Subnet::new(IpAddr([192, 168, 78, 0]), IpAddr([255, 255, 255, 0]));
    let node0_ip = IpAddr([192, 168, 77, 0]);
    let node1_77_ip = IpAddr([192, 168, 77, 1]);
    let node1_78_ip = IpAddr([192, 168, 78, 1]);
    let node2_ip = IpAddr([192, 168, 78, 2]);
    let node4_ip = IpAddr([192, 168, 78, 4]);

    // Create nodes
    let mut node0 = {
        let mut node = Node::new(0);
        let phy_id = 0;
        let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
        let ip_addr = node0_ip;

        let mut phy = Phy::new(phy_id, mac_addr, node.common.clone());
        phy.add_port(0).unwrap();
        let ether = Ether::new(
            mac_addr,
            ip_addr,
            phy_id,
            phy.port_ids(),
            node.common.clone(),
        );
        let ipif = IpIf::new(ip_addr, mac_addr, node.common.clone());
        let mut ip = Ip::new(vec![ip_addr], node.common.clone());

        ip.add_routing(subnet77, NextHop::Direct, node0_ip);
        ip.add_routing(subnet78, NextHop::Indirect(node1_77_ip), node0_ip);

        node.phys.insert(phy_id, phy);
        node.ethers.insert(mac_addr, ether);
        node.ipifs.insert(ip_addr, ipif);
        node.ip = Some(ip);

        node.on_recv = Some(Box::new(move |common, msg| {
            println!("Node#{} received: {:02X?}", common.node_id, msg);
            println!("Node#{} sending back to {}", common.node_id, node4_ip);
            common
                .op_tx
                .send(NextOp::IpSend {
                    protocol: Ip::IP_PROTOCOL_UDP,
                    dst_ip: node4_ip,
                    payload: msg.0,
                })
                .unwrap();
        }));

        node
    };

    let mut node1 = {
        let mut node = Node::new(1);

        {
            let phy_id = 0;
            let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
            let ip_addr = node1_77_ip;

            let mut phy = Phy::new(phy_id, mac_addr, node.common.clone());
            phy.add_port(0).unwrap();
            let ether = Ether::new(
                mac_addr,
                ip_addr,
                phy_id,
                phy.port_ids(),
                node.common.clone(),
            );
            let ipif = IpIf::new(ip_addr, mac_addr, node.common.clone());

            node.phys.insert(phy_id, phy);
            node.ethers.insert(mac_addr, ether);
            node.ipifs.insert(ip_addr, ipif);
        }
        {
            let phy_id = 1;
            let mac_addr = MacAddr([0x02, 0, 0, 0, 1, node.id as u8]);
            let ip_addr = node1_78_ip;

            let mut phy = Phy::new(phy_id, mac_addr, node.common.clone());
            phy.add_port(0).unwrap();
            let ether = Ether::new(
                mac_addr,
                ip_addr,
                phy_id,
                phy.port_ids(),
                node.common.clone(),
            );
            let ipif = IpIf::new(ip_addr, mac_addr, node.common.clone());

            node.phys.insert(phy_id, phy);
            node.ethers.insert(mac_addr, ether);
            node.ipifs.insert(ip_addr, ipif);
        }

        let mut ip = Ip::new(vec![node1_77_ip, node1_78_ip], node.common.clone());

        ip.add_routing(subnet77, NextHop::Direct, node1_77_ip);
        ip.add_routing(subnet78, NextHop::Direct, node1_78_ip);

        node.ip = Some(ip);

        node.on_recv = Some(Box::new(move |common, msg| {
            println!("Node#{} received: {:02X?}", common.node_id, msg);
        }));

        node
    };

    let mut node2 = {
        let mut node = Node::new(2);
        let phy_id = 0;
        let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
        let ip_addr = node2_ip;

        let mut phy = Phy::new(phy_id, mac_addr, node.common.clone());
        phy.add_port(0).unwrap();
        let ether = Ether::new(
            mac_addr,
            ip_addr,
            phy_id,
            phy.port_ids(),
            node.common.clone(),
        );
        let ipif = IpIf::new(ip_addr, mac_addr, node.common.clone());
        let mut ip = Ip::new(vec![ip_addr], node.common.clone());

        ip.add_routing(subnet77, NextHop::Indirect(node1_78_ip), node2_ip);
        ip.add_routing(subnet78, NextHop::Direct, node2_ip);

        node.phys.insert(phy_id, phy);
        node.ethers.insert(mac_addr, ether);
        node.ipifs.insert(ip_addr, ipif);
        node.ip = Some(ip);

        // initial data
        node.common
            .op_tx
            .send(NextOp::IpSend {
                protocol: Ip::IP_PROTOCOL_UDP,
                dst_ip: node0_ip,
                payload: vec![0, 1, 2, 3],
            })
            .unwrap();

        node
    };

    let mut node3 = {
        let mut node = Node::new(3);
        let phy_id = 0;
        let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
        let ip_addr = IpAddr([0xde, 0xad, 0xbe, 0xef]);

        let mut phy = Phy::new(phy_id, mac_addr, node.common.clone());
        phy.add_port(1).unwrap();
        phy.add_port(2).unwrap();
        phy.add_port(4).unwrap();
        let ether = Ether::new(
            mac_addr,
            ip_addr,
            phy_id,
            phy.port_ids(),
            node.common.clone(),
        );
        // let ipif = IpIf::new(ip_addr, mac_addr, node.common.clone());
        // let mut ip = Ip::new(vec![ip_addr], node.common.clone());

        // ip.add_routing(subnet77, NextHop::Indirect(node1_78_ip), node2_ip);
        // ip.add_routing(subnet78, NextHop::Direct, node2_ip);

        node.phys.insert(phy_id, phy);
        node.ethers.insert(mac_addr, ether);
        // node.ipifs.insert(ip_addr, ipif);
        // node.ip = Some(ip);

        // node.on_recv = Some(Box::new(move |common, msg| {
        //     println!("Node#{} received: {:02X?}", common.node_id, msg);
        // }));

        node
    };

    let mut node4 = {
        let mut node = Node::new(4);
        let phy_id = 0;
        let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
        let ip_addr = node4_ip;

        let mut phy = Phy::new(phy_id, mac_addr, node.common.clone());
        phy.add_port(0).unwrap();
        let ether = Ether::new(
            mac_addr,
            ip_addr,
            phy_id,
            phy.port_ids(),
            node.common.clone(),
        );
        let ipif = IpIf::new(ip_addr, mac_addr, node.common.clone());
        let mut ip = Ip::new(vec![ip_addr], node.common.clone());

        ip.add_routing(subnet77, NextHop::Indirect(node1_78_ip), node4_ip);
        ip.add_routing(subnet78, NextHop::Direct, node4_ip);

        node.phys.insert(phy_id, phy);
        node.ethers.insert(mac_addr, ether);
        node.ipifs.insert(ip_addr, ipif);
        node.ip = Some(ip);

        node.on_recv = Some(Box::new(move |common, msg| {
            println!("Node#{} received: {:02X?}", common.node_id, msg);
        }));

        node
    };

    // Connect ports
    let n0p0_0 = node0.get_port_mut(0, 0).unwrap();
    let n1p0_0 = node1.get_port_mut(0, 0).unwrap();
    n0p0_0.connect(n1p0_0).unwrap();
    let n1p1_0 = node1.get_port_mut(1, 0).unwrap();
    let n3p0_1 = node3.get_port_mut(0, 1).unwrap();
    n1p1_0.connect(n3p0_1).unwrap();
    let n3p0_2 = node3.get_port_mut(0, 2).unwrap();
    let n2p0_0 = node2.get_port_mut(0, 0).unwrap();
    n3p0_2.connect(n2p0_0).unwrap();
    let n3p0_4 = node3.get_port_mut(0, 4).unwrap();
    let n4p0_0 = node4.get_port_mut(0, 0).unwrap();
    n3p0_4.connect(n4p0_0).unwrap();

    let handles = [
        node0.run(),
        node1.run(),
        node2.run(),
        node3.run(),
        node4.run(),
    ];

    // Wait for nodes to finish
    for handle in handles {
        handle.join().unwrap();
    }
}
