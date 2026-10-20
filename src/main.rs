mod node;

use crate::node::ether::{Ether, MacAddr};
use crate::node::iface::Iface;
use crate::node::ip::{self, Ip, IpAddr, IpId, NextHop, Subnet};
use crate::node::port::{Port, PortId};
use crate::node::*;

fn main() {
    let subnet77 = Subnet::new(IpAddr([192, 168, 77, 0]), IpAddr([255, 255, 255, 0]));
    let subnet78 = Subnet::new(IpAddr([192, 168, 78, 0]), IpAddr([255, 255, 255, 0]));
    let node0_ip = IpAddr([192, 168, 77, 0]);
    let node1_ip = IpAddr([192, 168, 77, 1]);
    let node3_77_ip = IpAddr([192, 168, 77, 3]);
    let node3_78_ip = IpAddr([192, 168, 78, 3]);
    let node4_ip = IpAddr([192, 168, 78, 4]);

    fn add_iface(
        node: &mut Node,
        upper_ip_id: IpId,
        ip_addr: IpAddr,
        mac_addr: MacAddr,
        port_id: PortId,
    ) {
        let port = Port::new(port_id, mac_addr, node.common.clone());
        let ether = Ether::new(mac_addr, ip_addr, vec![port_id], node.common.clone());
        let iface = Iface::new(ip_addr, upper_ip_id, mac_addr, node.common.clone());
        node.ports.insert(port_id, port);
        node.ethers.insert(mac_addr, ether);
        node.ifaces.insert(ip_addr, iface);
    }

    /* Create nodes */

    // an end node that sends initial data
    let mut node0 = {
        let mut node = Node::new(0);
        let ip_id = 0;

        let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
        add_iface(&mut node, ip_id, node0_ip, mac_addr, 0);

        let mut ip = Ip::new(ip_id, vec![node0_ip], node.common.clone());
        ip.add_routing(subnet77, NextHop::Direct, node0_ip);
        ip.add_routing(subnet78, NextHop::Indirect(node3_77_ip), node0_ip);
        node.ips.insert(ip_id, ip);

        node.common.invoke(NextOp::IpSend {
            ip_id,
            arg: ip::SArg {
                protocol: Ip::IP_PROTOCOL_UDP,
                dst_ip: node4_ip,
                payload: vec![0, 1, 2, 3],
            },
        });

        node
    };

    // an end node, the final destination of the data
    let mut node1 = {
        let mut node = Node::new(1);
        let ip_id = 0;

        let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
        add_iface(&mut node, ip_id, node1_ip, mac_addr, 0);

        let mut ip = Ip::new(ip_id, vec![node1_ip], node.common.clone());
        ip.add_routing(subnet77, NextHop::Direct, node1_ip);
        ip.add_routing(subnet78, NextHop::Indirect(node3_77_ip), node1_ip);
        node.ips.insert(ip_id, ip);

        node.on_recv = Some(Box::new(move |common, data| {
            println!("Node#{} received: {:02X?}", common.node_id, data);
        }));

        node
    };

    // a switching hub (only with layers up to ethernet)
    let mut node2 = {
        let mut node = Node::new(2);
        let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
        let ip_addr = IpAddr([0xde, 0xad, 0xbe, 0xef]);

        let port0_id = 0;
        let port1_id = 1;
        let port3_id = 3;
        let port0 = Port::new(port0_id, mac_addr, node.common.clone());
        let port1 = Port::new(port1_id, mac_addr, node.common.clone());
        let port3 = Port::new(port3_id, mac_addr, node.common.clone());
        let ether = Ether::new(
            mac_addr,
            ip_addr,
            vec![port0_id, port1_id, port3_id],
            node.common.clone(),
        );

        node.ports.insert(port0_id, port0);
        node.ports.insert(port1_id, port1);
        node.ports.insert(port3_id, port3);
        node.ethers.insert(mac_addr, ether);

        node
    };

    // a router with two interfaces, connecting 192.168.77.0/24 and 192.168.78.0/24
    let mut node3 = {
        let mut node = Node::new(3);
        let ip_id = 0;

        let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
        add_iface(&mut node, ip_id, node3_77_ip, mac_addr, 0);

        let mac_addr = MacAddr([0x02, 0, 0, 0, 1, node.id as u8]);
        add_iface(&mut node, ip_id, node3_78_ip, mac_addr, 1);

        let mut ip = Ip::new(ip_id, vec![node3_77_ip, node3_78_ip], node.common.clone());
        ip.add_routing(subnet77, NextHop::Direct, node3_77_ip);
        ip.add_routing(subnet78, NextHop::Direct, node3_78_ip);
        node.ips.insert(ip_id, ip);

        node.on_recv = Some(Box::new(move |common, data| {
            println!("Node#{} received: {:02X?}", common.node_id, data);
        }));

        node
    };

    // an end node that sends the received data back to node1
    let mut node4 = {
        let mut node = Node::new(4);
        let ip_id = 0;

        let mac_addr = MacAddr([0x02, 0, 0, 0, 0, node.id as u8]);
        add_iface(&mut node, ip_id, node4_ip, mac_addr, 0);

        let mut ip = Ip::new(ip_id, vec![node4_ip], node.common.clone());
        ip.add_routing(subnet77, NextHop::Indirect(node3_78_ip), node4_ip);
        ip.add_routing(subnet78, NextHop::Direct, node4_ip);
        node.ips.insert(ip_id, ip);

        node.on_recv = Some(Box::new(move |common, data| {
            println!("Node#{} received: {:02X?}", common.node_id, data);
            println!("Node#{} sending back to {}", common.node_id, node1_ip);
            common.invoke(NextOp::IpSend {
                ip_id,
                arg: ip::SArg {
                    protocol: Ip::IP_PROTOCOL_UDP,
                    dst_ip: node1_ip,
                    payload: data,
                },
            });
        }));

        node
    };

    /* Connect ports */
    /* node0[77.0](p0)---(p0)node1(p3)---(p0)[77.2]node2[78.2](p1)---(p1)[78.4]node4 */
    /*                        (p1)                                                   */
    /*       node1[77.1](p0)---+                                                     */
    let n0p0 = node0.ports.get_mut(&0).unwrap();
    let n2p0 = node2.ports.get_mut(&0).unwrap();
    n0p0.connect(n2p0).unwrap();
    let n1p0 = node1.ports.get_mut(&0).unwrap();
    let n2p1 = node2.ports.get_mut(&1).unwrap();
    n1p0.connect(n2p1).unwrap();
    let n2p3 = node2.ports.get_mut(&3).unwrap();
    let n3p0 = node3.ports.get_mut(&0).unwrap();
    n2p3.connect(n3p0).unwrap();
    let n3p1 = node3.ports.get_mut(&1).unwrap();
    let n4p0 = node4.ports.get_mut(&0).unwrap();
    n3p1.connect(n4p0).unwrap();

    /* Run nodes */
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
