mod node;

use crate::node::ether::{Ether, MacAddr};
use crate::node::iface::Iface;
use crate::node::ip::{self, Ip, IpAddr, NextHop, Subnet};
use crate::node::port::Port;
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
        let port_id = 0;
        let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
        let ip_addr = node0_ip;
        let ip_id = 0;

        let port = Port::new(port_id, mac_addr, node.common.clone());
        let ether = Ether::new(mac_addr, ip_addr, vec![port_id], node.common.clone());
        let iface = Iface::new(ip_addr, ip_id, mac_addr, node.common.clone());
        let mut ip = Ip::new(ip_id, vec![ip_addr], node.common.clone());

        ip.add_routing(subnet77, NextHop::Direct, node0_ip);
        ip.add_routing(subnet78, NextHop::Indirect(node1_77_ip), node0_ip);

        node.ports.insert(port_id, port);
        node.ethers.insert(mac_addr, ether);
        node.ifaces.insert(ip_addr, iface);
        node.ips.insert(ip_id, ip);

        node.on_recv = Some(Box::new(move |common, data| {
            println!("Node#{} received: {:02X?}", common.node_id, data);
            println!("Node#{} sending back to {}", common.node_id, node4_ip);
            common.invoke(NextOp::IpSend {
                ip_id,
                arg: ip::SArg {
                    protocol: Ip::IP_PROTOCOL_UDP,
                    dst_ip: node4_ip,
                    payload: data,
                },
            });
        }));

        node
    };

    let mut node1 = {
        let mut node = Node::new(1);
        let ip_id = 0;

        {
            let port_id = 0;
            let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
            let ip_addr = node1_77_ip;

            let port = Port::new(port_id, mac_addr, node.common.clone());
            let ether = Ether::new(mac_addr, ip_addr, vec![port_id], node.common.clone());
            let iface = Iface::new(ip_addr, ip_id, mac_addr, node.common.clone());

            node.ports.insert(port_id, port);
            node.ethers.insert(mac_addr, ether);
            node.ifaces.insert(ip_addr, iface);
        }
        {
            let port_id = 1;
            let mac_addr = MacAddr([0x02, 0, 0, 0, 1, node.id as u8]);
            let ip_addr = node1_78_ip;

            let port = Port::new(port_id, mac_addr, node.common.clone());
            let ether = Ether::new(mac_addr, ip_addr, vec![port_id], node.common.clone());
            let iface = Iface::new(ip_addr, ip_id, mac_addr, node.common.clone());

            node.ports.insert(port_id, port);
            node.ethers.insert(mac_addr, ether);
            node.ifaces.insert(ip_addr, iface);
        }

        let mut ip = Ip::new(ip_id, vec![node1_77_ip, node1_78_ip], node.common.clone());

        ip.add_routing(subnet77, NextHop::Direct, node1_77_ip);
        ip.add_routing(subnet78, NextHop::Direct, node1_78_ip);

        node.ips.insert(ip_id, ip);

        node.on_recv = Some(Box::new(move |common, data| {
            println!("Node#{} received: {:02X?}", common.node_id, data);
        }));

        node
    };

    let mut node2 = {
        let mut node = Node::new(2);
        let port_id = 0;
        let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
        let ip_addr = node2_ip;
        let ip_id = 0;

        let port = Port::new(port_id, mac_addr, node.common.clone());
        let ether = Ether::new(mac_addr, ip_addr, vec![port_id], node.common.clone());
        let iface = Iface::new(ip_addr, ip_id, mac_addr, node.common.clone());
        let mut ip = Ip::new(ip_id, vec![ip_addr], node.common.clone());

        ip.add_routing(subnet77, NextHop::Indirect(node1_78_ip), node2_ip);
        ip.add_routing(subnet78, NextHop::Direct, node2_ip);

        node.ports.insert(port_id, port);
        node.ethers.insert(mac_addr, ether);
        node.ifaces.insert(ip_addr, iface);
        node.ips.insert(ip_id, ip);

        // initial data
        node.common.invoke(NextOp::IpSend {
            ip_id,
            arg: ip::SArg {
                protocol: Ip::IP_PROTOCOL_UDP,
                dst_ip: node0_ip,
                payload: vec![0, 1, 2, 3],
            },
        });

        node
    };

    let mut node3 = {
        let mut node = Node::new(3);
        let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
        let ip_addr = IpAddr([0xde, 0xad, 0xbe, 0xef]);
        // let ip_id = 0;

        let port1_id = 1;
        let port2_id = 2;
        let port4_id = 4;
        let port1 = Port::new(port1_id, mac_addr, node.common.clone());
        let port2 = Port::new(port2_id, mac_addr, node.common.clone());
        let port4 = Port::new(port4_id, mac_addr, node.common.clone());
        let ether = Ether::new(
            mac_addr,
            ip_addr,
            vec![port1_id, port2_id, port4_id],
            node.common.clone(),
        );
        // let iface = Iface::new(ip_addr, mac_addr, node.common.clone());
        // let mut ip = Ip::new(vec![ip_addr], node.common.clone());

        // ip.add_routing(subnet77, NextHop::Indirect(node1_78_ip), node2_ip);
        // ip.add_routing(subnet78, NextHop::Direct, node2_ip);

        node.ports.insert(port1_id, port1);
        node.ports.insert(port2_id, port2);
        node.ports.insert(port4_id, port4);
        node.ethers.insert(mac_addr, ether);
        // node.ifaces.insert(ip_addr, iface);
        // node.ips.insert(ip_id, ip);

        // node.on_recv = Some(Box::new(move |common, data| {
        //     println!("Node#{} received: {:02X?}", common.node_id, data);
        // }));

        node
    };

    let mut node4 = {
        let mut node = Node::new(4);
        let port_id = 0;
        let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
        let ip_addr = node4_ip;
        let ip_id = 0;

        let port = Port::new(port_id, mac_addr, node.common.clone());
        let ether = Ether::new(mac_addr, ip_addr, vec![port_id], node.common.clone());
        let iface = Iface::new(ip_addr, ip_id, mac_addr, node.common.clone());
        let mut ip = Ip::new(ip_id, vec![ip_addr], node.common.clone());

        ip.add_routing(subnet77, NextHop::Indirect(node1_78_ip), node4_ip);
        ip.add_routing(subnet78, NextHop::Direct, node4_ip);

        node.ports.insert(port_id, port);
        node.ethers.insert(mac_addr, ether);
        node.ifaces.insert(ip_addr, iface);
        node.ips.insert(ip_id, ip);

        node.on_recv = Some(Box::new(move |common, data| {
            println!("Node#{} received: {:02X?}", common.node_id, data);
        }));

        node
    };

    // Connect ports
    let n0p0 = node0.ports.get_mut(&0).unwrap();
    let n1p0 = node1.ports.get_mut(&0).unwrap();
    n0p0.connect(n1p0).unwrap();
    let n1p1 = node1.ports.get_mut(&1).unwrap();
    let n3p1 = node3.ports.get_mut(&1).unwrap();
    n1p1.connect(n3p1).unwrap();
    let n3p2 = node3.ports.get_mut(&2).unwrap();
    let n2p0 = node2.ports.get_mut(&0).unwrap();
    n3p2.connect(n2p0).unwrap();
    let n3p4 = node3.ports.get_mut(&4).unwrap();
    let n4p0 = node4.ports.get_mut(&0).unwrap();
    n3p4.connect(n4p0).unwrap();

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
