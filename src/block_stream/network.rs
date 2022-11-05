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
    essid: String,
    quality: u8,
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
        renderer: Renderer,
    ) -> Result<Self> {
        renderer
            .lock()
            .unwrap()
            .register_template_string(&name, template)?;
        Ok(Self {
            name,
            device,
            messages,
            renderer,
        })
    }

    fn get_initial_output(&self) -> Result<String> {
        let mut path = std::path::PathBuf::from("/sys/class/net");
        path.push(&self.device);
        path.push("operstate");
        let operstate = std::fs::read_to_string(path)?.trim().to_string();
        let data = self.build_block_data(operstate);
        let output = self.renderer.lock().unwrap().render(&self.name, &data)?;
        Ok(output)
    }

    fn build_block_data(&self, operstate: String) -> BlockData {
        match iwlib::get_wireless_info(self.device.clone()) {
            Some(info) => BlockData {
                device: self.device.clone(),
                operstate,
                wireless: true,
                essid: info.wi_essid,
                quality: info.wi_quality,
            },
            None => BlockData {
                device: self.device.clone(),
                operstate,
                wireless: false,
                essid: "".to_string(),
                quality: 0,
            },
        }
    }

    fn parse_message(&self, message: NetlinkMessage<RtnlMessage>) -> Option<BlockData> {
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
                        if link_matches {
                            if let Some(operstate) = operstate {
                                return Some(self.build_block_data(operstate));
                            }
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
                    if let Some(data) = self.parse_message(message) {
                        let output = self.renderer.lock().unwrap().render(&self.name, &data)?;
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

        let initial_output = block.get_initial_output()?;
        let first_run = stream::once(async { (name, Ok(initial_output)) });

        let stream = stream::unfold(block, move |mut block| async {
            let result = block.wait_for_output().await.transpose()?;
            Some(((block.name.clone(), result), block))
        });

        Ok(Box::pin(first_run.chain(stream)))
    }
}
