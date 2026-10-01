mod arp;

use std::collections::HashMap;
use std::collections::VecDeque;
use std::fmt::Display;
use std::ops::Deref;

use self::arp::{Arp, ArpPacket};
use crate::node::DbgInfo;
use crate::node::ether;
use crate::node::ether::{Ether, MacAddr};

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub(crate) struct IpAddr(pub(crate) [u8; 4]);

impl IpAddr {
    fn mask_with(&self, mask: Self) -> Self {
        Self([
            self[0] & mask[0],
            self[1] & mask[1],
            self[2] & mask[2],
            self[3] & mask[3],
        ])
    }
}

impl Display for IpAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{}.{}.{}.{}",
            self[0], self[1], self[2], self[3]
        ))
    }
}

impl Deref for IpAddr {
    type Target = [u8; 4];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<IpAddr> for [u8; 4] {
    fn from(value: IpAddr) -> Self {
        value.0
    }
}

impl<'a> From<&'a IpAddr> for &'a [u8] {
    fn from(value: &'a IpAddr) -> Self {
        &value.0
    }
}

pub(crate) struct Subnet {
    nw_addr: IpAddr,
    mask: IpAddr,
}

impl Subnet {
    pub(crate) fn new(ip_addr: IpAddr, mask: IpAddr) -> Self {
        let nw_addr = ip_addr.mask_with(mask);
        Self { nw_addr, mask }
    }

    fn contains(&self, ip_addr: IpAddr) -> bool {
        ip_addr.mask_with(self.mask) == self.nw_addr
    }
}

#[derive(Debug)]
pub(crate) struct Msg(pub(crate) Vec<u8>);

pub(crate) struct IpIf {
    pub(crate) ip_addr: IpAddr,
    arp_table: HashMap<IpAddr, MacAddr>,
    arp_wait_queue: VecDeque<(IpAddr, Vec<u8>)>,
    pub(crate) ether: Ether,
}

impl IpIf {
    pub(crate) fn new(ip_addr: IpAddr, ether: Ether) -> Self {
        IpIf {
            ip_addr,
            arp_table: HashMap::new(),
            arp_wait_queue: VecDeque::new(),
            ether,
        }
    }

    fn try_recv(&mut self) -> Option<Msg> {
        loop {
            let msg = self.ether.try_recv()?;
            if let Some(msg) = self.handle(msg) {
                return Some(msg);
            }
        }
    }

    #[allow(dead_code)]
    fn recv(&mut self) -> Msg {
        loop {
            let msg = self.ether.recv();
            if let Some(msg) = self.handle(msg) {
                return msg;
            }
        }
    }

    fn handle(&mut self, msg: ether::Msg) -> Option<Msg> {
        let ether::Msg { ether_type, data } = msg;
        match ether_type {
            Ether::ETHERTYPE_ARP => {
                Arp(self).handle(&data[0..ArpPacket::U8_REPR_SIZE]);
                while let Some((dest_ip, data)) = self.arp_wait_queue.pop_front() {
                    println!("{} re-sending saved req", self.dbg_header());
                    self.send(dest_ip, &data);
                }
                return None;
            }
            Ether::ETHERTYPE_IPV4 => {
                todo!("{} received IPv4 packet: {:02x?}", self.dbg_header(), data);
            }
            Ether::ETHERTYPE_LOCAL1 => {
                return Some(Msg(data));
            }
            _ => {
                eprintln!(
                    "{} received unsupported EtherType: {:#04x}",
                    self.dbg_header(),
                    ether_type
                );
                return None;
            }
        }
    }

    fn send(&mut self, dest_ip: IpAddr, data: &[u8]) {
        if let Some(&mac_addr) = Arp(self).get_mac_addr(dest_ip) {
            println!(
                "{} sending data to MAC addr: {} for IP: {}",
                self.dbg_header(),
                mac_addr,
                dest_ip
            );
            self.ether.send(mac_addr, Ether::ETHERTYPE_IPV4, data);
        } else {
            self.arp_wait_queue.push_back((dest_ip, data.into()));
            println!("{} saved send req temporarily", self.dbg_header());
            println!("{} sending arp req to {}", self.dbg_header(), dest_ip);
            Arp(self).send_arp_request(dest_ip);
        }
    }
}

impl Display for IpIf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IP-IF")
    }
}

impl DbgInfo for IpIf {
    fn node_id(&self) -> u32 {
        self.ether.node_id()
    }
}

struct RoutingTable(Vec<(Subnet, IpAddr)>);

impl RoutingTable {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn add(&mut self, subnet: Subnet, ifip: IpAddr) {
        self.0.push((subnet, ifip));
    }

    fn resolve(&self, ip_addr: IpAddr) -> Option<IpAddr> {
        self.0.iter().find_map(|(subnet, ifip)| {
            if subnet.contains(ip_addr) {
                Some(*ifip)
            } else {
                None
            }
        })
    }
}

pub(crate) struct Ip {
    ifs: Vec<IpIf>,
    routing_table: RoutingTable,
}

impl Ip {
    pub(crate) fn new(ifs: Vec<IpIf>) -> Self {
        Self {
            ifs,
            routing_table: RoutingTable::new(),
        }
    }

    pub(crate) fn add_routing(&mut self, subnet: Subnet, ifip: IpAddr) {
        self.routing_table.add(subnet, ifip);
    }

    #[allow(dead_code)]
    pub(crate) fn try_recv(&mut self) -> Option<Msg> {
        for ipif in self.ifs.iter_mut() {
            let msg = ipif.try_recv();
            if msg.is_some() {
                return msg;
            }
            // if None, continue to the next ipif
        }
        // All None. Give up
        None
    }

    pub(crate) fn recv(&mut self) -> Msg {
        loop {
            for ipif in self.ifs.iter_mut() {
                let msg = ipif.try_recv(); // non-blocking check
                if let Some(msg) = msg {
                    return msg;
                }
                // if None, continue to the next ipif
            }
            // All None. Retry forever
        }
    }

    pub(crate) fn send(&mut self, dest_ip: IpAddr, data: &[u8]) {
        if let Some(if_ip) = self.routing_table.resolve(dest_ip) {
            let ipif = self
                .ifs
                .iter_mut()
                .find(|ipif| ipif.ip_addr == if_ip)
                .unwrap();
            ipif.send(dest_ip, data);
        } else {
            panic!(
                "{} failed to resolve routing to {}",
                self.dbg_header(),
                dest_ip
            )
        }
    }
}

impl Display for Ip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IP")
    }
}

impl DbgInfo for Ip {
    fn node_id(&self) -> u32 {
        self.ifs[0].node_id()
    }
}
