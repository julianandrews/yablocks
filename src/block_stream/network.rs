use anyhow::Result;
use futures::{stream, StreamExt};
use netlink_packet_route::{
    constants::{RTNLGRP_IPV4_IFADDR, RTNLGRP_IPV6_IFADDR, RTNLGRP_LINK},
    link::nlas::Nla,
    NetlinkMessage, NetlinkPayload, RtnlMessage,
};
use rtnetlink::sys::{AsyncSocket, SocketAddr};

use super::{BlockStream, BlockStreamConfig};
use crate::RENDERER;

static NL_GRP: u32 =
    1 << (RTNLGRP_LINK - 1) | 1 << (RTNLGRP_IPV4_IFADDR - 1) | 1 << (RTNLGRP_IPV6_IFADDR - 1);

type Receiver =
    futures::channel::mpsc::UnboundedReceiver<(NetlinkMessage<RtnlMessage>, SocketAddr)>;

struct Block {
    name: String,
    device: String,
    messages: Receiver,
}

impl Block {
    fn parse_message(&self, message: NetlinkMessage<RtnlMessage>) -> Option<String> {
        if let NetlinkPayload::InnerMessage(message) = message.payload {
            match message {
                RtnlMessage::NewLink(link_message)
                | RtnlMessage::DelLink(link_message)
                | RtnlMessage::SetLink(link_message)
                | RtnlMessage::NewLinkProp(link_message)
                | RtnlMessage::DelLinkProp(link_message) => {
                    let mut link_matches = false;
                    let mut operstate = None;
                    for nla in link_message.nlas {
                        match nla {
                            Nla::IfName(name) | Nla::AltIfName(name) => {
                                if name == self.device {
                                    link_matches = true
                                }
                            }
                            Nla::OperState(state) => {
                                operstate = Some(format!("{:?}", state).to_lowercase())
                            }
                            _ => (),
                        }
                        if link_matches && operstate.is_some() {
                            return operstate;
                        }
                    }
                }
                _ => (),
            }
        }
        None
    }

    async fn wait_for_output(&mut self) -> Option<Result<String>> {
        loop {
            let (message, _) = self.messages.next().await?;
            if let Some(operstate) = self.parse_message(message) {
                let data = BlockData::read(self.device.clone(), operstate);
                return Some(RENDERER.render(&self.name, data));
            }
        }
    }
}

impl BlockStreamConfig for crate::config::NetworkConfig {
    fn to_stream(self, name: String) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{operstate}}".to_string());
        RENDERER.add_template(&name, &template)?;

        let (mut conn, mut _handle, messages) = rtnetlink::new_connection()?;
        let addr = SocketAddr::new(0, NL_GRP);
        conn.socket_mut().socket_mut().bind(&addr)?;
        tokio::spawn(conn);

        let block = Block {
            name: name.clone(),
            device: self.device.clone(),
            messages,
        };
        let first_run = stream::once(async move {
            let result = render_device_state(&name, self.device).await;
            (name, result)
        });
        let stream = stream::unfold(block, move |mut block| async {
            let result = block.wait_for_output().await?;
            Some(((block.name.clone(), result), block))
        });

        Ok(Box::pin(first_run.chain(stream)))
    }
}

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData {
    device: String,
    operstate: String,
    wireless: bool,
    essid: Option<String>,
    quality: Option<u8>,
}

impl BlockData {
    fn read(device: String, operstate: String) -> Self {
        match BlockData::get_wireless_info(&device) {
            Ok(Some((essid, quality))) => BlockData {
                device,
                operstate,
                wireless: true,
                essid,
                quality,
            },
            _ => BlockData {
                device,
                operstate,
                wireless: false,
                essid: None,
                quality: None,
            },
        }
    }

    fn get_wireless_info(device: &str) -> Result<Option<(Option<String>, Option<u8>)>> {
        let interfaces = nl80211::Socket::connect()?.get_interfaces_info()?;
        for interface in interfaces {
            if let Some(bytes) = &interface.name {
                let found_device = String::from_utf8_lossy(bytes);
                if found_device.trim_end_matches(char::from(0)) == device {
                    let essid = interface.ssid.as_ref().map(nl80211::parse_string);
                    let station = interface.get_station_info()?;
                    let signal_strength = station.average_signal.as_ref().map(nl80211::parse_i8);
                    let quality =
                        signal_strength.map(|dbm| 2 * (dbm.max(-100).min(-50) + 100) as u8);
                    return Ok(Some((essid, quality)));
                }
            }
        }
        Ok(None)
    }
}

async fn render_device_state(name: &str, device: String) -> Result<String> {
    let path: std::path::PathBuf = ["/sys/class/net", &device, "operstate"].iter().collect();
    let operstate = tokio::fs::read_to_string(path).await?.trim().to_string();
    let data = BlockData::read(device, operstate);
    RENDERER.render(name, data)
}
