use std::fmt::Display;
use std::ops::Deref;

use super::IpIf;
use crate::node::DbgInfo;
use crate::node::ether::{Ether, MacAddr};
use crate::node::ip::IpAddr;

pub(super) struct ArpPacket {
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
    pub(super) const U8_REPR_SIZE: usize = 28;

    const HTYPE_ETHERNET: u16 = 1;

    pub(super) const OP_REQUEST: u16 = 1;
    pub(super) const OP_REPLY: u16 = 2;

    pub(super) fn new(
        opcode: u16,
        dest_ip: IpAddr,
        sender_mac: MacAddr,
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

pub(super) struct Arp<'a>(pub(super) &'a mut IpIf);

impl Arp<'_> {
    pub(super) fn get_mac_addr(&self, ip: IpAddr) -> Option<&MacAddr> {
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

    fn send_arp_reply(&self, dest_mac: MacAddr, dest_ip: IpAddr, sender_ip: IpAddr) {
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
