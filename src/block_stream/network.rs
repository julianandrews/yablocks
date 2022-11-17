use anyhow::Result;
use futures::{stream, StreamExt};
use netlink_packet_route::{
    constants::{RTNLGRP_IPV4_IFADDR, RTNLGRP_IPV6_IFADDR, RTNLGRP_LINK},
    link::nlas::Nla,
    NetlinkMessage, NetlinkPayload, RtnlMessage,
};
use rtnetlink::sys::{AsyncSocket, SocketAddr};

use super::{BlockStream, BlockStreamConfig, Renderer};

static NL_GRP: u32 =
    1 << (RTNLGRP_LINK - 1) | 1 << (RTNLGRP_IPV4_IFADDR - 1) | 1 << (RTNLGRP_IPV6_IFADDR - 1);

type Receiver =
    futures::channel::mpsc::UnboundedReceiver<(NetlinkMessage<RtnlMessage>, SocketAddr)>;

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData {
    device: String,
    operstate: String,
    wireless: bool,
    essid: Option<String>,
    quality: Option<u8>,
}

struct Block {
    name: String,
    device: String,
    messages: Receiver,
    renderer: Renderer,
}

impl Block {
    fn new(
        name: String,
        template: String,
        device: String,
        messages: Receiver,
        mut renderer: Renderer,
    ) -> Result<Self> {
        renderer.add_template(&name, &template)?;
        Ok(Self {
            name,
            device,
            messages,
            renderer,
        })
    }

    async fn get_initial_output(&self) -> Result<String> {
        let mut path = std::path::PathBuf::from("/sys/class/net");
        path.push(&self.device);
        path.push("operstate");
        let operstate = tokio::fs::read_to_string(path).await?.trim().to_string();
        let data = self.build_block_data(operstate);
        let output = self.renderer.render(&self.name, data)?;
        Ok(output)
    }

    fn build_block_data(&self, operstate: String) -> BlockData {
        match self.build_block_data_with_wifi(&operstate) {
            Ok(Some(data)) => data,
            _ => BlockData {
                device: self.device.clone(),
                operstate,
                wireless: false,
                essid: None,
                quality: None,
            },
        }
    }

    fn build_block_data_with_wifi(&self, operstate: &str) -> Result<Option<BlockData>> {
        let interfaces = nl80211::Socket::connect()?.get_interfaces_info()?;
        for interface in interfaces {
            if let Some(data) = &interface.name {
                let device = String::from_utf8_lossy(data)
                    .trim_end_matches(char::from(0))
                    .to_owned()
                    .to_string();
                if device == self.device {
                    let essid = interface.ssid.as_ref().map(nl80211::parse_string);
                    let station = interface.get_station_info()?;
                    let signal_strength = station.average_signal.as_ref().map(nl80211::parse_i8);
                    let quality =
                        signal_strength.map(|dbm| 2 * (dbm.max(-100).min(-50) + 100) as u8);
                    return Ok(Some(BlockData {
                        device,
                        operstate: operstate.to_string(),
                        wireless: true,
                        essid,
                        quality,
                    }));
                }
            }
        }
        Ok(None)
    }

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

    async fn wait_for_output(&mut self) -> Result<Option<String>> {
        loop {
            match self.messages.next().await {
                Some((message, _)) => {
                    if let Some(operstate) = self.parse_message(message) {
                        let data = self.build_block_data(operstate);
                        let output = self.renderer.render(&self.name, data)?;
                        return Ok(Some(output));
                    }
                }
                None => return Ok(None),
            };
        }
    }
}

impl BlockStreamConfig for crate::config::NetworkConfig {
    fn to_stream(self, name: String, renderer: Renderer) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{operstate}}".to_string());
        let (mut conn, mut _handle, messages) = rtnetlink::new_connection()?;
        let addr = SocketAddr::new(0, NL_GRP);
        conn.socket_mut().socket_mut().bind(&addr)?;
        tokio::spawn(conn);

        let block = Block::new(name.clone(), template, self.device, messages, renderer)?;

        let initial_output = futures::executor::block_on(block.get_initial_output())?;
        let first_run = stream::once(async { (name, Ok(initial_output)) });

        let stream = stream::unfold(block, move |mut block| async {
            let result = block.wait_for_output().await.transpose()?;
            Some(((block.name.clone(), result), block))
        });

        Ok(Box::pin(first_run.chain(stream)))
    }
}
