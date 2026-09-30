mod arp {
    use std::fmt::Display;
    use std::ops::Deref;

    use super::IpIf;
    use crate::node::DbgInfo;
    use crate::node::ether::Ether;
    use crate::node::ip::IpAddr;

    pub(super) struct ArpPacket {
        hardware_type: u16,
        protocol_type: u16,
        hardware_size: u8,
        protocol_size: u8,
        opcode: u16,
        sender_mac: [u8; 6],
        sender_ip: IpAddr,
        target_mac: [u8; 6],
        target_ip: IpAddr,
    }

    impl ArpPacket {
        pub(super) const U8_REPR_SIZE: usize = 28;

        const HTYPE_ETHERNET: u16 = 1;

        pub(super) const OP_REQUEST: u16 = 1;
        pub(super) const OP_REPLY: u16 = 2;

        pub(super) fn new(
            opcode: u16,
            dest_ip: IpAddr,
            sender_mac: [u8; 6],
            sender_ip: IpAddr,
        ) -> Self {
            ArpPacket {
                hardware_type: Self::HTYPE_ETHERNET,
                protocol_type: Ether::ETHERTYPE_IPV4,
                hardware_size: 6,
                protocol_size: 4,
                opcode,
                sender_mac,
                sender_ip,
                target_mac: [0; 6],
                target_ip: dest_ip,
            }
        }
    }

    impl From<ArpPacket> for Vec<u8> {
        fn from(packet: ArpPacket) -> Self {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&packet.hardware_type.to_be_bytes());
            bytes.extend_from_slice(&packet.protocol_type.to_be_bytes());
            bytes.push(packet.hardware_size);
            bytes.push(packet.protocol_size);
            bytes.extend_from_slice(&packet.opcode.to_be_bytes());
            bytes.extend_from_slice(&packet.sender_mac);
            bytes.extend_from_slice(packet.sender_ip.deref());
            bytes.extend_from_slice(&packet.target_mac);
            bytes.extend_from_slice(packet.target_ip.deref());
            bytes
        }
    }

    impl TryFrom<&[u8]> for ArpPacket {
        type Error = &'static str;

        fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
            if bytes.len() != Self::U8_REPR_SIZE {
                return Err("Invalid ARP packet length");
            }

            let hardware_type = u16::from_be_bytes(
                <[u8; 2]>::try_from(&bytes[0..2]).map_err(|_| "Failed to parse hardware_type")?,
            );
            let protocol_type = u16::from_be_bytes(
                <[u8; 2]>::try_from(&bytes[2..4]).map_err(|_| "Failed to parse protocol_type")?,
            );
            let hardware_size = bytes[4];
            let protocol_size = bytes[5];
            let opcode = u16::from_be_bytes(
                <[u8; 2]>::try_from(&bytes[6..8]).map_err(|_| "Failed to parse opcode")?,
            );
            let sender_mac =
                <[u8; 6]>::try_from(&bytes[8..14]).map_err(|_| "Failed to parse sender_mac")?;
            let sender_ip = IpAddr(
                <[u8; 4]>::try_from(&bytes[14..18]).map_err(|_| "Failed to parse sender_ip")?,
            );
            let target_mac =
                <[u8; 6]>::try_from(&bytes[18..24]).map_err(|_| "Failed to parse target_mac")?;
            let target_ip = IpAddr(
                <[u8; 4]>::try_from(&bytes[24..28]).map_err(|_| "Failed to parse target_ip")?,
            );

            Ok(ArpPacket {
                hardware_type,
                protocol_type,
                hardware_size,
                protocol_size,
                opcode,
                sender_mac,
                sender_ip,
                target_mac,
                target_ip,
            })
        }
    }

    pub(super) struct Arp<'a>(pub(super) &'a mut IpIf);

    impl Arp<'_> {
        pub(super) fn get_mac_addr(&self, ip: IpAddr) -> Option<&[u8; 6]> {
            self.0.arp_table.get(&ip)
        }

        pub(super) fn send_arp_request(&self, dest_ip: IpAddr) {
            let arp_packet = ArpPacket::new(
                ArpPacket::OP_REQUEST,
                dest_ip,
                self.0.ether.mac_addr,
                self.0.ip_addr,
            );
            self.0.ether.send(
                Ether::MAC_ADDR_BROADCAST,
                Ether::ETHERTYPE_ARP,
                &Vec::from(arp_packet),
            );
        }

        fn send_arp_reply(&self, dest_mac: [u8; 6], dest_ip: IpAddr, sender_ip: IpAddr) {
            let arp_packet = ArpPacket::new(
                ArpPacket::OP_REPLY,
                dest_ip,
                self.0.ether.mac_addr,
                sender_ip,
            );
            self.0
                .ether
                .send(dest_mac, Ether::ETHERTYPE_ARP, &Vec::from(arp_packet));
        }

        pub(super) fn handle(&mut self, arp_packet_data: &[u8]) {
            match ArpPacket::try_from(arp_packet_data) {
                Err(e) => {
                    eprintln!(
                        "{} received invalid packet: {}, {:02x?}",
                        self.dbg_header(),
                        e,
                        arp_packet_data
                    );
                    return;
                }
                Ok(ArpPacket {
                    hardware_type,
                    protocol_type,
                    hardware_size,
                    protocol_size,
                    opcode,
                    sender_mac,
                    sender_ip,
                    target_mac: _,
                    target_ip,
                }) => {
                    if hardware_type != ArpPacket::HTYPE_ETHERNET
                        || protocol_type != Ether::ETHERTYPE_IPV4
                        || hardware_size != 6
                        || protocol_size != 4
                    {
                        eprintln!(
                            "{} unsupported metadata: HW type: {:#04x}, proto type: {:#04x}, HW size: {}, proto size: {}",
                            self.dbg_header(),
                            hardware_type,
                            protocol_type,
                            hardware_size,
                            protocol_size
                        );
                        return;
                    }

                    self.0.arp_table.insert(sender_ip, sender_mac);

                    match opcode {
                        ArpPacket::OP_REQUEST => {
                            println!("{} req received from {}", self.dbg_header(), sender_ip);
                            if target_ip == self.0.ip_addr {
                                println!("{} sending reply to {}", self.dbg_header(), sender_ip);
                                self.send_arp_reply(sender_mac, sender_ip, self.0.ip_addr);
                            }
                        }
                        ArpPacket::OP_REPLY => {
                            println!("{} reply received from {}", self.dbg_header(), sender_ip);
                            return;
                        }
                        _ => {
                            eprintln!(
                                "{} received from {} but invalid op code: {:#04x}",
                                self.dbg_header(),
                                sender_ip,
                                opcode
                            );
                            return;
                        }
                    }
                }
            }
        }
    }

    impl Display for Arp<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "ARP")
        }
    }

    impl DbgInfo for Arp<'_> {
        fn node_id(&self) -> u32 {
            self.0.node_id()
        }
    }
}

use std::collections::HashMap;
use std::collections::VecDeque;
use std::fmt::Display;
use std::ops::Deref;

use self::arp::{Arp, ArpPacket};
use crate::node::DbgInfo;
use crate::node::ether;
use crate::node::ether::Ether;

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
    arp_table: HashMap<IpAddr, [u8; 6]>,
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
                "{} sending data to MAC addr: {:02x?} for IP: {}",
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
