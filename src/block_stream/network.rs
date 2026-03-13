use std::net::IpAddr;

use anyhow::Result;
use futures::{stream, StreamExt, TryStreamExt};
use netlink_packet_route::{
    constants::{
        RTNLGRP_IPV4_IFADDR, RTNLGRP_IPV4_ROUTE, RTNLGRP_IPV6_IFADDR, RTNLGRP_IPV6_ROUTE,
        RTNLGRP_LINK,
    },
    link::nlas::Nla,
    NetlinkMessage, NetlinkPayload, RtnlMessage,
};
use rtnetlink::sys::{AsyncSocket, SocketAddr};

use super::{BlockStream, BlockStreamConfig};
use crate::RENDERER;

static NL_GRP: u32 = 1 << (RTNLGRP_LINK - 1)
    | 1 << (RTNLGRP_IPV4_IFADDR - 1)
    | 1 << (RTNLGRP_IPV6_IFADDR - 1)
    | 1 << (RTNLGRP_IPV4_ROUTE - 1)
    | 1 << (RTNLGRP_IPV6_ROUTE - 1);

type Receiver =
    futures::channel::mpsc::UnboundedReceiver<(NetlinkMessage<RtnlMessage>, SocketAddr)>;

struct Block {
    name: String,
    device: String,
    messages: Receiver,
}

struct WifiInfo {
    essid: Option<String>,
    quality: Option<u8>,
    signal_dbm: Option<i8>,
    frequency: Option<u32>,
}

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData {
    device: String,
    operstate: String,
    wireless: bool,
    essid: Option<String>,
    quality: Option<u8>,
    signal_dbm: Option<i8>,
    frequency: Option<u32>,
    ipv4_addresses: Vec<IpAddr>,
    ipv6_addresses: Vec<IpAddr>,
    ipv4_gateway: Option<IpAddr>,
    ipv6_gateway: Option<IpAddr>,
}

enum IpVersion {
    V4,
    V6,
}

impl IpVersion {
    fn parse_addr(&self, addr: &[u8]) -> Option<IpAddr> {
        match self {
            IpVersion::V4 if addr.len() == 4 => {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(addr);
                Some(IpAddr::V4(std::net::Ipv4Addr::from(bytes)))
            }
            IpVersion::V6 if addr.len() == 16 => {
                let mut bytes = [0u8; 16];
                bytes.copy_from_slice(addr);
                let ip = IpAddr::V6(std::net::Ipv6Addr::from(bytes));
                if ip.is_loopback() || ip.is_unspecified() {
                    None
                } else {
                    Some(ip)
                }
            }
            _ => None,
        }
    }

    fn route_file(&self) -> &'static str {
        match self {
            IpVersion::V4 => "/proc/net/route",
            IpVersion::V6 => "/proc/net/ipv6_route",
        }
    }

    fn default_route_prefix(&self) -> &str {
        match self {
            IpVersion::V4 => "00000000",
            IpVersion::V6 => "00000000000000000000000000000000",
        }
    }

    fn parse_gateway_hex(&self, hex: &str) -> Option<IpAddr> {
        match self {
            IpVersion::V4 if hex.len() == 8 => {
                let ip_val = u32::from_str_radix(hex, 16).ok()?;
                let ip_val = ip_val.swap_bytes();
                let ip = IpAddr::V4(std::net::Ipv4Addr::from(ip_val));
                if ip.is_unspecified() {
                    None
                } else {
                    Some(ip)
                }
            }
            IpVersion::V6 if hex.len() == 32 => {
                let val = u128::from_str_radix(hex, 16).ok()?;
                let ip = IpAddr::V6(std::net::Ipv6Addr::from(val.to_be_bytes()));
                if ip.is_unspecified() {
                    None
                } else {
                    Some(ip)
                }
            }
            _ => None,
        }
    }
}

impl Block {
    fn parse_message(&self, message: NetlinkMessage<RtnlMessage>) -> Option<u32> {
        match message.payload {
            NetlinkPayload::InnerMessage(
                RtnlMessage::NewLink(msg)
                | RtnlMessage::DelLink(msg)
                | RtnlMessage::SetLink(msg)
                | RtnlMessage::NewLinkProp(msg)
                | RtnlMessage::DelLinkProp(msg),
            ) => {
                for nla in msg.nlas {
                    if let Nla::IfName(name) | Nla::AltIfName(name) = nla {
                        if name == self.device {
                            return Some(msg.header.index);
                        }
                    }
                }
                None
            }
            NetlinkPayload::InnerMessage(
                RtnlMessage::NewAddress(msg) | RtnlMessage::DelAddress(msg),
            ) => Some(msg.header.index),
            NetlinkPayload::InnerMessage(
                RtnlMessage::NewRoute(msg) | RtnlMessage::DelRoute(msg),
            ) => {
                for nla in &msg.nlas {
                    if let netlink_packet_route::route::nlas::Nla::Oif(oif) = nla {
                        return Some(*oif);
                    }
                }
                None
            }
            _ => None,
        }
    }

    async fn wait_for_output(&mut self) -> Option<Result<String>> {
        loop {
            let (message, _) = self.messages.next().await?;
            if let Some(ifindex) = self.parse_message(message) {
                let data = BlockData::read(ifindex, &self.device).await;
                return Some(RENDERER.render(&self.name, data));
            }
        }
    }
}

impl BlockData {
    async fn read(ifindex: u32, device: &str) -> Self {
        let operstate = Self::get_operstate(device).await;
        let wireless_info = Self::get_wireless_info(device).ok().flatten();
        let ipv4_addresses = Self::get_ip_addresses(ifindex, IpVersion::V4).await;
        let ipv6_addresses = Self::get_ip_addresses(ifindex, IpVersion::V6).await;
        let ipv4_gateway = Self::get_gateway(device, IpVersion::V4).await;
        let ipv6_gateway = Self::get_gateway(device, IpVersion::V6).await;

        let (essid, quality, signal_dbm, frequency) = wireless_info
            .map_or((None, None, None, None), |w| {
                (w.essid, w.quality, w.signal_dbm, w.frequency)
            });

        BlockData {
            device: device.to_string(),
            operstate,
            wireless: essid.is_some() || quality.is_some(),
            essid,
            quality,
            signal_dbm,
            frequency,
            ipv4_addresses,
            ipv6_addresses,
            ipv4_gateway,
            ipv6_gateway,
        }
    }

    async fn get_operstate(device: &str) -> String {
        let path: std::path::PathBuf = ["/sys/class/net", device, "operstate"].iter().collect();
        tokio::fs::read_to_string(path)
            .await
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string())
    }

    fn get_wireless_info(device: &str) -> Result<Option<WifiInfo>> {
        let interfaces = nl80211::Socket::connect()?.get_interfaces_info()?;
        for interface in interfaces {
            if let Some(bytes) = &interface.name {
                let found_device = String::from_utf8_lossy(bytes);
                if found_device.trim_end_matches(char::from(0)) == device {
                    let essid = interface.ssid.as_ref().map(nl80211::parse_string);
                    let station = interface.get_station_info()?;
                    let signal_strength = station.average_signal.as_ref().map(nl80211::parse_i8);
                    let quality = signal_strength.map(|dbm| 2 * (dbm.clamp(-100, -50) + 100) as u8);
                    let frequency = interface.frequency.as_ref().map(nl80211::parse_u32);
                    return Ok(Some(WifiInfo {
                        essid,
                        quality,
                        signal_dbm: signal_strength,
                        frequency,
                    }));
                }
            }
        }
        Ok(None)
    }

    async fn get_ip_addresses(ifindex: u32, version: IpVersion) -> Vec<IpAddr> {
        let (conn, handle, _) = match rtnetlink::new_connection() {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        tokio::spawn(conn);

        let mut addresses = vec![];
        let mut stream = handle.address().get().execute().map_ok(|msg| msg);
        while let Some(Ok(addr_msg)) = stream.next().await {
            if addr_msg.header.index != ifindex {
                continue;
            }
            for nla in addr_msg.nlas {
                if let netlink_packet_route::address::nlas::Nla::Address(addr) = nla {
                    if let Some(ip) = version.parse_addr(&addr) {
                        addresses.push(ip);
                    }
                }
            }
        }

        addresses
    }

    async fn get_gateway(device: &str, version: IpVersion) -> Option<IpAddr> {
        let content = tokio::fs::read_to_string(version.route_file()).await.ok()?;
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 && parts[0] == device && parts[1] == version.default_route_prefix()
            {
                if let Some(ip) = version.parse_gateway_hex(parts[2]) {
                    return Some(ip);
                }
            }
        }
        None
    }
}

impl BlockStreamConfig for crate::config::NetworkConfig {
    fn to_stream(self, name: String) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{operstate}}".to_string());
        RENDERER.add_template(&name, &template)?;

        let (mut conn, _, messages) = rtnetlink::new_connection()?;
        let addr = SocketAddr::new(0, NL_GRP);
        conn.socket_mut().socket_mut().bind(&addr)?;
        tokio::spawn(conn);

        let device = self.device.clone();
        let ifindex = match std::fs::read_to_string(format!("/sys/class/net/{device}/ifindex")) {
            Ok(s) => match s.trim().parse() {
                Ok(i) => i,
                Err(_) => return Err(anyhow::anyhow!("failed to parse ifindex")),
            },
            Err(_) => return Err(anyhow::anyhow!("failed to read ifindex")),
        };
        let name_clone = name.clone();
        let first_run = stream::once(async move {
            let data = BlockData::read(ifindex, &device).await;
            let result = RENDERER.render(&name_clone, data);
            (name_clone, result)
        });

        let block = Block {
            name: name.clone(),
            device: self.device.clone(),
            messages,
        };
        let stream = stream::unfold(block, move |mut block| async {
            let result = block.wait_for_output().await?;
            Some(((block.name.clone(), result), block))
        });

        Ok(Box::pin(first_run.chain(stream)))
    }
}
