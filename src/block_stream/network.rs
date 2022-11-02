use anyhow::Result;
use futures::stream;
use futures::StreamExt;
use netlink_packet_route::{
    constants::{RTNLGRP_IPV4_IFADDR, RTNLGRP_IPV6_IFADDR, RTNLGRP_LINK},
    link::nlas::Nla,
    NetlinkMessage, NetlinkPayload, RtnlMessage,
};
use rtnetlink::{
    new_connection,
    sys::{AsyncSocket, SocketAddr},
};

use super::{BlockStream, BlockStreamConfig, Renderer};

static NL_GRP: u32 =
    1 << (RTNLGRP_LINK - 1) | 1 << (RTNLGRP_IPV4_IFADDR - 1) | 1 << (RTNLGRP_IPV6_IFADDR - 1);

type Receiver = futures::channel::mpsc::UnboundedReceiver<(
    NetlinkMessage<RtnlMessage>,
    rtnetlink::sys::SocketAddr,
)>;

struct Block {
    name: String,
    device: String,
    messages: Receiver,
    renderer: Renderer,
}

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData {
    device: String,
    operstate: String,
    wireless: bool,
    essid: String,
    quality: u8,
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

    fn initial_state(&self) -> Result<BlockData> {
        let mut path = std::path::PathBuf::from("/sys/class/net");
        path.push(&self.device);
        path.push("operstate");
        let operstate = std::fs::read_to_string(path)?.trim().to_string();
        Ok(self.build_block_data(operstate))
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

    fn render(&self, data: &BlockData) -> Result<String> {
        let output = self.renderer.lock().unwrap().render(&self.name, &data)?;
        Ok(output)
    }
}

impl BlockStreamConfig for crate::config::NetworkConfig {
    fn to_stream(self, name: String, renderer: Renderer) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{operstate}}".to_string());
        let (mut conn, mut _handle, messages) = new_connection()?;
        let addr = SocketAddr::new(0, NL_GRP);
        conn.socket_mut().socket_mut().bind(&addr)?;
        tokio::spawn(conn);

        let state = Block::new(name.clone(), template, self.device, messages, renderer)?;

        let initial_contents = state.render(&state.initial_state()?)?;
        let first_run = stream::once(async { (name, initial_contents) });

        let stream = stream::unfold(state, move |mut state| async {
            loop {
                let (message, _) = match state.messages.next().await {
                    Some(message) => message,
                    None => return None,
                };
                if let Some(data) = state.parse_message(message) {
                    let output = match state.render(&data) {
                        Ok(output) => output,
                        Err(error) => {
                            eprintln!("Error rendering template: {:?}", error);
                            "Error".to_string()
                        }
                    };
                    return Some(((state.name.clone(), output), state));
                }
            }
        });

        Ok(Box::pin(first_run.chain(stream)))
    }
}
