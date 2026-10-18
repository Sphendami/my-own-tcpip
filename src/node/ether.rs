use std::collections::HashMap;
use std::fmt::Display;
use std::ops::Deref;

use crate::node::iface;
use crate::node::ip::IpAddr;
use crate::node::port::{self, PortId};
use crate::node::{Common, DbgInfo, NextOp};

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub(crate) struct MacAddr(pub(crate) [u8; 6]);

impl Display for MacAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self[0], self[1], self[2], self[3], self[4], self[5]
        ))
    }
}

impl Deref for MacAddr {
    type Target = [u8; 6];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<MacAddr> for [u8; 6] {
    fn from(value: MacAddr) -> Self {
        value.0
    }
}

impl<'a> From<&'a MacAddr> for &'a [u8] {
    fn from(value: &'a MacAddr) -> Self {
        &value.0
    }
}

struct Header {
    dst_mac: MacAddr,
    src_mac: MacAddr,
    ethertype: u16,
}

impl Header {
    fn new(dst_mac: MacAddr, src_mac: MacAddr, ethertype: u16) -> Self {
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
        bytes.extend_from_slice(header.dst_mac.deref());
        bytes.extend_from_slice(header.src_mac.deref());
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

        let dst_mac =
            MacAddr(<[u8; 6]>::try_from(&bytes[0..6]).map_err(|_| "Failed to parse dst_mac")?);
        let src_mac =
            MacAddr(<[u8; 6]>::try_from(&bytes[6..12]).map_err(|_| "Failed to parse src_mac")?);
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

pub(crate) struct RArg {
    pub(crate) port_id: PortId,
    pub(crate) data: Vec<u8>,
}

pub(crate) struct SArg {
    pub(crate) dst_mac: MacAddr,
    pub(crate) ethertype: u16,
    pub(crate) payload: Vec<u8>,
}

pub(crate) struct Ether {
    pub(crate) mac_addr: MacAddr,
    mac_addr_table: HashMap<MacAddr, PortId>,
    upper: IpAddr,
    lowers: Vec<PortId>,
    common: Common,
}

impl Ether {
    pub(crate) const MAC_ADDR_BROADCAST: MacAddr = MacAddr([0xFF; 6]);

    pub(crate) const ETHERTYPE_IPV4: u16 = 0x0800;
    pub(crate) const ETHERTYPE_ARP: u16 = 0x0806;
    // pub(crate) const ETHERTYPE_LOCAL1: u16 = 0x88B5;

    pub(crate) fn new(
        mac_addr: MacAddr,
        upper: IpAddr,
        lowers: Vec<PortId>,
        common: Common,
    ) -> Self {
        Ether {
            mac_addr,
            mac_addr_table: HashMap::new(),
            upper,
            lowers,
            common,
        }
    }

    fn resolve_port_ids(&self, dst_mac: MacAddr, except: Option<PortId>) -> Vec<PortId> {
        match self.mac_addr_table.get(&dst_mac) {
            Some(&port_id) => vec![port_id],
            // flooding except the port it came from
            None => self
                .lowers
                .clone()
                .into_iter()
                .filter(|&id| Some(id) != except)
                .collect(),
        }
    }

    pub(crate) fn recv(&mut self, arg: RArg) {
        if let Some(arg) = self.handle(arg) {
            self.common.invoke(NextOp::IfaceReceive {
                iface_id: self.upper,
                arg,
            });
        }
    }

    fn handle(&mut self, arg: RArg) -> Option<iface::RArg> {
        let RArg {
            port_id,
            data: frame,
        } = arg;

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

        // Update MAC address table
        self.mac_addr_table.insert(header.src_mac, port_id);

        println!(
            "{} received frame on port {}: dst_mac={}, src_mac={}, ethertype={:#X}, frame_len={}",
            self.dbg_header(),
            port_id,
            header.dst_mac,
            header.src_mac,
            header.ethertype,
            frame.len()
        );

        if header.dst_mac != self.mac_addr || header.dst_mac == Self::MAC_ADDR_BROADCAST {
            // forward the frame (except the port it came from)
            let fwd_port_ids = self.resolve_port_ids(header.dst_mac, Some(port_id));
            if fwd_port_ids.len() != 0 {
                println!(
                    "{} forwarding the frame on port {:?}",
                    self.dbg_header(),
                    fwd_port_ids,
                );
            }

            for fwd_port_id in fwd_port_ids {
                self.common.invoke(NextOp::PortSend {
                    port_id: fwd_port_id,
                    arg: port::SArg {
                        data: frame.clone(),
                    },
                });
            }
        }

        if header.dst_mac == self.mac_addr || header.dst_mac == Self::MAC_ADDR_BROADCAST {
            Some(iface::RArg {
                ether_type: header.ethertype,
                data: payload.to_vec(),
            })
        } else {
            None
        }
    }

    pub(crate) fn send(&self, arg: SArg) {
        let SArg {
            dst_mac,
            ethertype,
            payload,
        } = arg;

        let header = Header::new(dst_mac, self.mac_addr, ethertype);
        let mut frame = Vec::new();
        frame.extend_from_slice(&Vec::from(header));
        frame.extend_from_slice(&payload);

        // Pad the frame to a minimum of 64 bytes
        while frame.len() < 64 - 4 {
            frame.push(0);
        }

        let crc = crc32(&frame);
        frame.extend_from_slice(&crc.to_be_bytes());

        let port_ids = self.resolve_port_ids(dst_mac, None);
        if port_ids.len() == 0 {
            eprintln!(
                "{} no port found for dst_mac={} (ignore the send)",
                self.dbg_header(),
                dst_mac
            );
            return;
        }

        println!(
            "{} sending frame on port {:?}: dst_mac={}, src_mac={}, ethertype={:#X}, frame_len={}",
            self.dbg_header(),
            port_ids,
            dst_mac,
            self.mac_addr,
            ethertype,
            frame.len()
        );

        for port_id in port_ids {
            self.common.invoke(NextOp::PortSend {
                port_id,
                arg: port::SArg {
                    data: frame.clone(),
                },
            });
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
        self.common.node_id
    }
}
