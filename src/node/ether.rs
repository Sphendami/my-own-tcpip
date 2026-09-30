use std::collections::HashMap;
use std::fmt::Display;

use crate::node::DbgInfo;
use crate::node::phy;
use crate::node::phy::Phy;

struct Header {
    dst_mac: [u8; 6],
    src_mac: [u8; 6],
    ethertype: u16,
}

impl Header {
    fn new(dst_mac: [u8; 6], src_mac: [u8; 6], ethertype: u16) -> Self {
        Header {
            dst_mac,
            src_mac,
            ethertype,
        }
    }
}

impl From<Header> for Vec<u8> {
    fn from(header: Header) -> Self {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&header.dst_mac);
        bytes.extend_from_slice(&header.src_mac);
        bytes.extend_from_slice(&header.ethertype.to_be_bytes());
        bytes
    }
}

impl TryFrom<&[u8]> for Header {
    type Error = &'static str;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() != 14 {
            return Err("Invalid Ethernet header length");
        }

        let dst_mac = <[u8; 6]>::try_from(&bytes[0..6]).map_err(|_| "Failed to parse dst_mac")?;
        let src_mac = <[u8; 6]>::try_from(&bytes[6..12]).map_err(|_| "Failed to parse src_mac")?;
        let ethertype = u16::from_be_bytes(
            <[u8; 2]>::try_from(&bytes[12..14]).map_err(|_| "Failed to parse ethertype")?,
        );

        Ok(Header {
            dst_mac,
            src_mac,
            ethertype,
        })
    }
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0;

    for &byte in data {
        crc ^= (byte as u32) << 24;

        for _ in 0..8 {
            if crc & (1 << 31) != 0 {
                crc = (crc << 1) ^ 0x04C1_1DB7;
            } else {
                crc <<= 1;
            }
        }
    }

    crc
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_crc32() {
        let data = [1, 2, 3, 4];
        let crc = crc32(&data);

        let crc2 = crc32(&[&data[..], &crc.to_be_bytes()].concat());
        assert_eq!(crc2, 0);
    }

    #[test]
    fn test_crc32_str() {
        let data = b"Hello, world!";
        let crc = crc32(data);

        let crc2 = crc32(&[&data[..], &crc.to_be_bytes()].concat());
        assert_eq!(crc2, 0);
    }
}

pub(crate) struct Msg {
    pub(crate) ether_type: u16,
    pub(crate) data: Vec<u8>,
}

pub(crate) struct Ether {
    pub(crate) mac_addr: [u8; 6],
    mac_addr_table: HashMap<[u8; 6], phy::PortId>,
    pub(crate) phy: Phy,
}

impl Ether {
    pub(crate) const MAC_ADDR_BROADCAST: [u8; 6] = [0xFF; 6];

    pub(crate) const ETHERTYPE_IPV4: u16 = 0x0800;
    pub(crate) const ETHERTYPE_ARP: u16 = 0x0806;
    pub(crate) const ETHERTYPE_LOCAL1: u16 = 0x88B5;

    pub(crate) fn new(mac_addr: [u8; 6], phy: Phy) -> Self {
        Ether {
            mac_addr,
            mac_addr_table: HashMap::new(),
            phy,
        }
    }

    pub(crate) fn try_recv(&mut self) -> Option<Msg> {
        loop {
            let msg = self.phy.try_recv()?;
            if let Some(msg) = self.handle(msg) {
                return Some(msg);
            }
        }
    }

    pub(crate) fn recv(&mut self) -> Msg {
        loop {
            let msg = self.phy.recv();
            if let Some(msg) = self.handle(msg) {
                return msg;
            }
        }
    }

    fn handle(&mut self, msg: phy::Msg) -> Option<Msg> {
        let phy::Msg {
            port_id,
            data: frame,
        } = msg;

        if frame.len() < 64 {
            eprintln!(
                "{} received frame too short on port {}",
                self.dbg_header(),
                port_id
            );
            return None;
        }
        let header = &frame[..14];
        let payload = &frame[14..frame.len() - 4];

        let header = match Header::try_from(header) {
            Ok(header) => header,
            Err(e) => {
                eprintln!(
                    "{} received frame with invalid header on port {}: {}",
                    self.dbg_header(),
                    port_id,
                    e
                );
                return None;
            }
        };

        if crc32(&frame) != 0 {
            eprintln!(
                "{} received frame with invalid CRC on port {}",
                self.dbg_header(),
                port_id
            );
            return None;
        }

        if header.dst_mac != self.mac_addr && header.dst_mac != Self::MAC_ADDR_BROADCAST {
            eprintln!(
                "{} received frame not for this MAC on port {}: dst_mac={:?}",
                self.dbg_header(),
                port_id,
                header.dst_mac
            );
            return None;
        }

        // Update MAC address table
        self.mac_addr_table.insert(header.src_mac, port_id);

        println!(
            "{} received frame on port {}: dst_mac={:02X?}, src_mac={:02X?}, ethertype={:#X}, frame_len={}",
            self.dbg_header(),
            port_id,
            header.dst_mac,
            header.src_mac,
            header.ethertype,
            frame.len()
        );

        Some(Msg {
            ether_type: header.ethertype,
            data: payload.to_vec(),
        })
    }

    pub(crate) fn send(&self, dst_mac: [u8; 6], ethertype: u16, payload: &[u8]) {
        let header = Header::new(dst_mac, self.mac_addr, ethertype);
        let mut frame = Vec::new();
        frame.extend_from_slice(&Vec::from(header));
        frame.extend_from_slice(payload);

        // Pad the frame to a minimum of 64 bytes
        while frame.len() < 64 - 4 {
            frame.push(0);
        }

        let crc = crc32(&frame);
        frame.extend_from_slice(&crc.to_be_bytes());

        let port_id = self.mac_addr_table.get(&dst_mac);

        println!(
            "{} sending frame on port {:?}: dst_mac={:02X?}, src_mac={:02X?}, ethertype={:#X}, frame_len={}",
            self.dbg_header(),
            port_id,
            dst_mac,
            self.mac_addr,
            ethertype,
            frame.len()
        );

        if let Some(&port_id) = port_id {
            self.phy.send(port_id, frame);
        } else {
            // Flooding
            self.phy.send_to_all(frame);
        }
    }
}

impl Display for Ether {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Ether")
    }
}

impl DbgInfo for Ether {
    fn node_id(&self) -> u32 {
        self.phy.node_id()
    }
}
