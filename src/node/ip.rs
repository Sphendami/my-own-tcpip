use std::fmt::Display;
use std::ops::Deref;

use crate::node::iface;
use crate::node::{Common, DbgInfo, NextOp};

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

#[derive(Clone, Copy)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IpV4Header {
    version: u8,
    ihl: u8,  // Internet Header Length
    dscp: u8, // Differentiated Services Code Point
    ecn: u8,  // Explicit Congestion Notification
    total_length: u16,
    identification: u16,
    dont_fragment: bool,
    more_fragments: bool,
    fragment_offset: u16,
    ttl: u8, // Time to Live
    protocol: u8,
    checksum: u16,
    source_ip: IpAddr,
    dest_ip: IpAddr,
}

impl IpV4Header {
    const U8_REPR_SIZE: u16 = 20;

    fn new(source_ip: IpAddr, dest_ip: IpAddr, protocol: u8, payload_size: u16) -> Self {
        let total_length = Self::U8_REPR_SIZE + payload_size;
        let mut header = Self {
            version: 4,
            ihl: 5, // 5 * 4 = 20 bytes
            dscp: 0,
            ecn: 0,
            total_length,
            identification: 0, // For simplicity, set to 0
            dont_fragment: true,
            more_fragments: false,
            fragment_offset: 0,
            ttl: 64, // Default TTL
            protocol,
            checksum: 0,
            source_ip,
            dest_ip,
        };
        header.set_checksum();
        header
    }

    fn calc_checksum(&self) -> u16 {
        let mut sum: u32 = <Vec<u8>>::from(self)
            .chunks(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]) as u32)
            .sum();

        // Add carry bits
        while (sum >> 16) != 0 {
            sum = (sum & 0xffff) + (sum >> 16);
        }

        (!sum) as u16
    }

    fn set_checksum(&mut self) {
        // checksum field must be zeroed before calculating the checksum
        self.checksum = 0;
        self.checksum = self.calc_checksum();
    }

    fn decrease_ttl(&mut self) -> bool {
        if self.ttl > 0 {
            self.ttl -= 1;
            self.set_checksum();
            true
        } else {
            false
        }
    }
}

impl From<&IpV4Header> for Vec<u8> {
    fn from(header: &IpV4Header) -> Self {
        let mut bytes = Vec::new();
        let version_ihl = (header.version << 4) | (header.ihl & 0x0F);
        bytes.push(version_ihl);
        let dscp_ecn = (header.dscp << 2) | (header.ecn & 0x03);
        bytes.push(dscp_ecn);
        bytes.extend_from_slice(&header.total_length.to_be_bytes());
        bytes.extend_from_slice(&header.identification.to_be_bytes());
        let flags_fragment_offset = ((header.dont_fragment as u16) << 14)
            | ((header.more_fragments as u16) << 13)
            | (header.fragment_offset & 0x1FFF);
        bytes.extend_from_slice(&flags_fragment_offset.to_be_bytes());
        bytes.push(header.ttl);
        bytes.push(header.protocol);
        bytes.extend_from_slice(&header.checksum.to_be_bytes());
        bytes.extend_from_slice(header.source_ip.deref());
        bytes.extend_from_slice(header.dest_ip.deref());
        bytes
    }
}

impl TryFrom<&[u8]> for IpV4Header {
    type Error = &'static str;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() < IpV4Header::U8_REPR_SIZE as usize {
            return Err("IPv4 header must be at least 20 bytes");
        }
        let version_ihl = bytes[0];
        let dscp_ecn = bytes[1];
        let flags_fragment_offset = u16::from_be_bytes([bytes[6], bytes[7]]);

        Ok(IpV4Header {
            version: version_ihl >> 4,
            ihl: version_ihl & 0x0F,
            dscp: dscp_ecn >> 2,
            ecn: dscp_ecn & 0x03,
            total_length: u16::from_be_bytes([bytes[2], bytes[3]]),
            identification: u16::from_be_bytes([bytes[4], bytes[5]]),
            dont_fragment: (flags_fragment_offset >> 14) & 1 == 1,
            more_fragments: (flags_fragment_offset >> 13) & 1 == 1,
            fragment_offset: flags_fragment_offset & 0x1FFF,
            ttl: bytes[8],
            protocol: bytes[9],
            checksum: u16::from_be_bytes([bytes[10], bytes[11]]),
            source_ip: IpAddr([bytes[12], bytes[13], bytes[14], bytes[15]]),
            dest_ip: IpAddr([bytes[16], bytes[17], bytes[18], bytes[19]]),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_ip_header_serdes() {
        let source_ip = IpAddr([192, 168, 1, 1]);
        let dest_ip = IpAddr([192, 168, 1, 2]);
        let protocol = Ip::IP_PROTOCOL_UDP;
        let total_length = 40; // Example length

        let header = IpV4Header::new(source_ip, dest_ip, protocol, total_length);
        let bytes: Vec<u8> = (&header).into();

        let deserialized_header = IpV4Header::try_from(bytes.as_slice()).unwrap();
        assert_eq!(header, deserialized_header);
    }

    #[test]
    fn test_ip_header_checksum() {
        let source_ip = IpAddr([192, 168, 1, 1]);
        let dest_ip = IpAddr([192, 168, 1, 2]);
        let protocol = Ip::IP_PROTOCOL_UDP;
        let total_length = 40; // Example length

        let header = IpV4Header::new(source_ip, dest_ip, protocol, total_length);
        assert_eq!(header.calc_checksum(), 0);
    }
}

#[derive(Debug)]
pub(crate) struct RArg(pub(crate) Vec<u8>);

pub(crate) struct SArg {
    pub(crate) protocol: u8,
    pub(crate) dst_ip: IpAddr,
    pub(crate) payload: Vec<u8>,
}

#[derive(Clone, Copy)]
pub(crate) enum NextHop {
    Direct,
    Indirect(IpAddr),
}

struct RoutingTable(Vec<(Subnet, NextHop, IpAddr)>);

impl RoutingTable {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn add(&mut self, subnet: Subnet, next_hop: NextHop, iface_id: IpAddr) {
        self.0.push((subnet, next_hop, iface_id));
    }

    fn resolve(&self, ip_addr: IpAddr) -> Option<(NextHop, IpAddr)> {
        self.0.iter().find_map(|(subnet, next_hop, iface_id)| {
            if subnet.contains(ip_addr) {
                Some((*next_hop, *iface_id))
            } else {
                None
            }
        })
    }
}

pub(crate) type IpId = u32;

pub(crate) struct Ip {
    id: IpId,
    routing_table: RoutingTable,
    lowers: Vec<IpAddr>,
    common: Common,
}

impl Ip {
    // pub(crate) const IP_PROTOCOL_TCP: u8 = 0x06;
    pub(crate) const IP_PROTOCOL_UDP: u8 = 0x11;

    pub(crate) fn new(id: IpId, lowers: Vec<IpAddr>, common: Common) -> Self {
        Self {
            id,
            routing_table: RoutingTable::new(),
            lowers,
            common,
        }
    }

    pub(crate) fn add_routing(&mut self, subnet: Subnet, next_hop: NextHop, iface_id: IpAddr) {
        self.routing_table.add(subnet, next_hop, iface_id);
    }

    fn resolve_routing(&self, dest_ip: IpAddr) -> Option<(IpAddr, NextHop)> {
        let (next_hop, iface_id) = self.routing_table.resolve(dest_ip)?;
        self.lowers
            .iter()
            .find(|id| **id == iface_id)
            .map(|iface_id| (*iface_id, next_hop))
    }

    fn is_my_ip(&self, ip_addr: IpAddr) -> bool {
        self.lowers.iter().any(|addr| *addr == ip_addr)
    }

    pub(crate) fn recv(&self, arg: RArg) -> Option<Vec<u8>> {
        if let Some(arg) = self.handle(arg) {
            Some(arg)
        } else {
            None
        }
    }

    fn handle(&self, arg: RArg) -> Option<Vec<u8>> {
        let RArg(data) = arg;

        let h_len = IpV4Header::U8_REPR_SIZE as usize;
        let mut header = match IpV4Header::try_from(&data[0..h_len]) {
            Ok(header) => header,
            Err(e) => {
                eprintln!("{} failed to parse IPv4 header: {}", self.dbg_header(), e);
                return None;
            }
        };
        // drop the header and padding bytes
        let payload = &data[h_len..header.total_length as usize];
        println!(
            "{} received packet: src_ip={}, dest_ip={}, protocol=0x{:02x}, len={}",
            self.dbg_header(),
            header.source_ip,
            header.dest_ip,
            header.protocol,
            payload.len()
        );

        if self.is_my_ip(header.dest_ip) {
            println!("{} the packet is for me", self.dbg_header());
            return Some(payload.to_vec());
        } else {
            // Not for me. Forward it.
            let decreased = header.decrease_ttl();
            if !decreased {
                eprintln!("{} received packet with TTL=0, dropping", self.dbg_header());
                return None;
            }
            println!("{} forwarding the packet", self.dbg_header());
            self.send_with_header(header, payload);
            return None;
        }
    }

    pub(crate) fn send(&self, arg: SArg) {
        let SArg {
            protocol,
            dst_ip: dest_ip,
            payload: data,
        } = arg;

        let (iface_id, next_hop) = self
            .resolve_routing(dest_ip)
            .expect(&format!("failed to resolve routing to {}", dest_ip));
        let header = IpV4Header::new(iface_id, dest_ip, protocol, data.len() as u16);
        let next_ip = match next_hop {
            NextHop::Direct => header.dest_ip,
            NextHop::Indirect(ip) => ip,
        };
        let header_bytes = Vec::from(&header);
        let data = [header_bytes.as_slice(), data.as_slice()].concat();
        self.common.invoke(NextOp::IfaceSend {
            iface_id,
            arg: iface::SArg { next_ip, data },
        });
    }

    fn send_with_header(&self, header: IpV4Header, data: &[u8]) {
        let (iface_id, next_hop) = self
            .resolve_routing(header.dest_ip)
            .expect(&format!("failed to resolve routing to {}", header.dest_ip));
        let next_ip = match next_hop {
            NextHop::Direct => header.dest_ip,
            NextHop::Indirect(ip) => ip,
        };
        let data = [&<Vec<u8>>::from(&header), data].concat();
        self.common.invoke(NextOp::IfaceSend {
            iface_id,
            arg: iface::SArg { next_ip, data },
        });
    }
}

impl Display for Ip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IP#{}", self.id)
    }
}

impl DbgInfo for Ip {
    fn node_id(&self) -> u32 {
        self.common.node_id
    }
}
