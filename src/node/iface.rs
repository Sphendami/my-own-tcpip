use std::collections::{HashMap, VecDeque};
use std::fmt::Display;
use std::ops::Deref;

use crate::node::ether::{self, Ether, MacAddr};
use crate::node::ip::IpAddr;
use crate::node::{Common, DbgInfo, NextOp};

struct ArpPacket {
    hardware_type: u16,
    protocol_type: u16,
    hardware_size: u8,
    protocol_size: u8,
    opcode: u16,
    sender_mac: MacAddr,
    sender_ip: IpAddr,
    target_mac: MacAddr,
    target_ip: IpAddr,
}

impl ArpPacket {
    const U8_REPR_SIZE: usize = 28;

    const HTYPE_ETHERNET: u16 = 1;

    const OP_REQUEST: u16 = 1;
    const OP_REPLY: u16 = 2;

    fn new(opcode: u16, dest_ip: IpAddr, sender_mac: MacAddr, sender_ip: IpAddr) -> Self {
        ArpPacket {
            hardware_type: Self::HTYPE_ETHERNET,
            protocol_type: Ether::ETHERTYPE_IPV4,
            hardware_size: 6,
            protocol_size: 4,
            opcode,
            sender_mac,
            sender_ip,
            target_mac: MacAddr([0; 6]),
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
        bytes.extend_from_slice(packet.sender_mac.deref());
        bytes.extend_from_slice(packet.sender_ip.deref());
        bytes.extend_from_slice(packet.target_mac.deref());
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
            MacAddr(<[u8; 6]>::try_from(&bytes[8..14]).map_err(|_| "Failed to parse sender_mac")?);
        let sender_ip =
            IpAddr(<[u8; 4]>::try_from(&bytes[14..18]).map_err(|_| "Failed to parse sender_ip")?);
        let target_mac =
            MacAddr(<[u8; 6]>::try_from(&bytes[18..24]).map_err(|_| "Failed to parse target_mac")?);
        let target_ip =
            IpAddr(<[u8; 4]>::try_from(&bytes[24..28]).map_err(|_| "Failed to parse target_ip")?);

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

enum ArpHandleResult {
    Request {
        sender_mac: MacAddr,
        sender_ip: IpAddr,
    },
    Reply {
        #[allow(dead_code)]
        sender_mac: MacAddr,
        sender_ip: IpAddr,
    },
}

struct Arp<'a>(&'a mut Iface);

impl Arp<'_> {
    fn get_mac_addr(&self, ip: IpAddr) -> Option<&MacAddr> {
        self.0.arp_table.get(&ip)
    }

    fn send_arp_request(&self, dest_ip: IpAddr) {
        let arp_packet =
            ArpPacket::new(ArpPacket::OP_REQUEST, dest_ip, self.0.lower, self.0.ip_addr);
        self.0
            .common
            .op_tx
            .send(NextOp::EtherSend {
                ether_id: self.0.lower,
                dst_mac: Ether::MAC_ADDR_BROADCAST,
                ethertype: Ether::ETHERTYPE_ARP,
                payload: Vec::from(arp_packet),
            })
            .unwrap();
    }

    fn send_arp_reply(&self, dest_mac: MacAddr, dest_ip: IpAddr) {
        let arp_packet = ArpPacket::new(ArpPacket::OP_REPLY, dest_ip, self.0.lower, self.0.ip_addr);
        self.0
            .common
            .op_tx
            .send(NextOp::EtherSend {
                ether_id: self.0.lower,
                dst_mac: dest_mac,
                ethertype: Ether::ETHERTYPE_ARP,
                payload: Vec::from(arp_packet),
            })
            .unwrap();
    }

    fn handle(&mut self, arp_packet_data: &[u8]) -> Option<ArpHandleResult> {
        match ArpPacket::try_from(arp_packet_data) {
            Err(e) => {
                eprintln!(
                    "{} received invalid packet: {}, {:02x?}",
                    self.dbg_header(),
                    e,
                    arp_packet_data
                );
                return None;
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
                    return None;
                }

                self.0.arp_table.insert(sender_ip, sender_mac);

                if target_ip != self.0.ip_addr {
                    eprintln!(
                        "{} received arp packet for {} but my IP is {}. dropping the packet.",
                        self.dbg_header(),
                        target_ip,
                        self.0.ip_addr
                    );
                    return None;
                }

                match opcode {
                    ArpPacket::OP_REQUEST => Some(ArpHandleResult::Request {
                        sender_mac,
                        sender_ip,
                    }),
                    ArpPacket::OP_REPLY => Some(ArpHandleResult::Reply {
                        sender_mac,
                        sender_ip,
                    }),
                    _ => {
                        eprintln!(
                            "{} received from {} but invalid op code: {:#04x}",
                            self.dbg_header(),
                            sender_ip,
                            opcode
                        );
                        return None;
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

pub(crate) struct Msg(pub(crate) Vec<u8>);

pub(crate) struct Iface {
    pub(crate) ip_addr: IpAddr,
    arp_table: HashMap<IpAddr, MacAddr>,
    arp_wait_queue: VecDeque<(IpAddr, Vec<u8>)>,
    lower: MacAddr,
    common: Common,
}

impl Iface {
    pub(crate) fn new(ip_addr: IpAddr, lower: MacAddr, common: Common) -> Self {
        Iface {
            ip_addr,
            arp_table: HashMap::new(),
            arp_wait_queue: VecDeque::new(),
            lower,
            common,
        }
    }

    pub(crate) fn recv(&mut self, msg: ether::Msg) {
        if let Some(msg) = self.handle(msg) {
            self.common.op_tx.send(NextOp::IpReceive { msg }).unwrap();
        }
    }

    fn handle(&mut self, msg: ether::Msg) -> Option<Msg> {
        let ether::Msg { ether_type, data } = msg;
        match ether_type {
            Ether::ETHERTYPE_ARP => {
                match Arp(self).handle(&data[0..ArpPacket::U8_REPR_SIZE])? {
                    ArpHandleResult::Request {
                        sender_mac,
                        sender_ip,
                    } => {
                        println!("{} received arp req from {}", self.dbg_header(), sender_ip);
                        println!("{} sending arp reply to {}", self.dbg_header(), sender_ip);
                        Arp(self).send_arp_reply(sender_mac, sender_ip);
                    }
                    ArpHandleResult::Reply {
                        sender_mac: _,
                        sender_ip,
                    } => {
                        println!(
                            "{} received arp reply from {}",
                            self.dbg_header(),
                            sender_ip
                        );
                        while let Some((next_ip, data)) = self.arp_wait_queue.pop_front() {
                            println!("{} re-sending saved req", self.dbg_header());
                            self.send(next_ip, &data);
                        }
                    }
                }
                return None;
            }
            Ether::ETHERTYPE_IPV4 => {
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

    pub(crate) fn send(&mut self, next_ip: IpAddr, data: &[u8]) {
        if let Some(&mac_addr) = Arp(self).get_mac_addr(next_ip) {
            println!(
                "{} sending data to MAC addr: {} for IP: {}",
                self.dbg_header(),
                mac_addr,
                next_ip
            );
            self.common
                .op_tx
                .send(NextOp::EtherSend {
                    ether_id: self.lower,
                    dst_mac: mac_addr,
                    ethertype: Ether::ETHERTYPE_IPV4,
                    payload: data.to_vec(),
                })
                .unwrap();
        } else {
            self.arp_wait_queue.push_back((next_ip, data.into()));
            println!("{} saved send req temporarily", self.dbg_header());
            println!("{} sending arp req to {}", self.dbg_header(), next_ip);
            Arp(self).send_arp_request(next_ip);
        }
    }
}

impl Display for Iface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IF#{}.{}", self.ip_addr.0[2], self.ip_addr.0[3])
    }
}

impl DbgInfo for Iface {
    fn node_id(&self) -> u32 {
        self.common.node_id
    }
}
