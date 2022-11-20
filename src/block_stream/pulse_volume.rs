use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

use anyhow::Result;
use futures::channel::mpsc::{Receiver, Sender};
use futures::{stream, StreamExt};
use pulse::callbacks::ListResult;
use pulse::context::introspect::SinkInfo;
use pulse::context::subscribe::InterestMaskSet;
use pulse::context::{Context, FlagSet as ContextFlagSet};
use pulse::mainloop::standard::IterateResult;
use pulse::mainloop::standard::Mainloop;
use pulse::proplist::Proplist;
use pulse::volume::Volume;

use super::{util::send_or_eprint, BlockStream, BlockStreamConfig};
use crate::RENDERER;

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData {
    sink_name: String,
    volume: u32,
    muted: bool,
}

struct Block {
    name: String,
    rx: Receiver<Result<BlockData>>,
}

impl Block {
    async fn wait_for_output(&mut self) -> Result<Option<String>> {
        let data = match self.rx.next().await {
            Some(data) => data?,
            None => return Ok(None),
        };
        let output = RENDERER.render(&self.name, data)?;
        Ok(Some(output))
    }
}

impl BlockStreamConfig for crate::config::PulseVolumeConfig {
    fn to_stream(self, name: String) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{volume}}".to_string());
        RENDERER.add_template(&name, &template)?;

        let (tx, rx) = futures::channel::mpsc::channel::<Result<BlockData>>(1);
        tokio::task::spawn_blocking(move || monitor_sink(self.sink_name, tx));

        let block = Block { name, rx };
        let stream = stream::unfold(block, move |mut block| async {
            let result = block.wait_for_output().await.transpose()?;
            Some(((block.name.clone(), result), block))
        });

        Ok(Box::pin(stream))
    }
}

pub struct PulseVolumeMonitor {
    mainloop: Rc<RefCell<Mainloop>>,
    context: Rc<RefCell<Context>>,
}

impl PulseVolumeMonitor {
    fn new() -> Result<Self> {
        let mut proplist =
            Proplist::new().ok_or_else(|| anyhow::anyhow!("Failed to build proplist"))?;
        proplist
            .set_str(
                pulse::proplist::properties::APPLICATION_NAME,
                env!("CARGO_BIN_NAME"),
            )
            .map_err(|_| anyhow::anyhow!("Failed to build proplist"))?;
        let mainloop =
            Rc::new(RefCell::new(Mainloop::new().ok_or_else(|| {
                anyhow::anyhow!("Failed to get pulse audio mainloop")
            })?));
        let context = Rc::new(RefCell::new(
            Context::new_with_proplist(
                mainloop.borrow().deref(),
                concat!(env!("CARGO_BIN_NAME"), "Context"),
                &proplist,
            )
            .ok_or_else(|| anyhow::anyhow!("Failed to get pulse audio context"))?,
        ));
        context
            .borrow_mut()
            .connect(None, ContextFlagSet::NOFLAGS, None)?;

        // Wait until pulse is ready
        loop {
            match mainloop.borrow_mut().iterate(false) {
                IterateResult::Quit(_) => {
                    return Err(anyhow::anyhow!("Pulse audio mainloop quit"));
                }
                IterateResult::Err(error) => {
                    return Err(anyhow::anyhow!("Pulse audio mainloop error: {:?}", error));
                }
                IterateResult::Success(_) => {}
            }
            match context.borrow().get_state() {
                pulse::context::State::Ready => break,
                pulse::context::State::Failed | pulse::context::State::Terminated => {
                    return Err(anyhow::anyhow!("Pulse audio failed to reach ready state"));
                }
                _ => {}
            }
        }
        Ok(Self { mainloop, context })
    }

    fn add_sink(&mut self, sink_name: Option<String>, mut tx: Sender<Result<BlockData>>) {
        // Send the initial volume state
        send_block_data(self.context.clone(), sink_name.clone(), tx.clone());
        let context_clone = self.context.clone();
        let tx_clone = tx.clone();
        self.context
            .borrow_mut()
            .set_subscribe_callback(Some(Box::new(move |_facility, _operation, _index| {
                send_block_data(context_clone.clone(), sink_name.clone(), tx_clone.clone());
            })));
        self.context
            .borrow_mut()
            .subscribe(InterestMaskSet::SINK, move |success| {
                if !success {
                    send_or_eprint(
                        Err(anyhow::anyhow!("Failed to subsribe to pulse audio events")),
                        &mut tx,
                    );
                }
            });
    }

    fn run(&mut self) -> Result<()> {
        self.mainloop.borrow_mut().run().map_err(|(err, _retval)| {
            anyhow::anyhow!("Failed to run pulse audio mainloop: {:?}", err)
        })?;
        Ok(())
    }
}

fn monitor_sink(sink_name: Option<String>, mut tx: Sender<Result<BlockData>>) {
    let mut monitor = match PulseVolumeMonitor::new() {
        Ok(monitor) => monitor,
        Err(error) => {
            send_or_eprint(
                Err(anyhow::anyhow!(
                    "Failed to construct pulse volume monitor: {:?}",
                    error
                )),
                &mut tx,
            );
            return;
        }
    };
    monitor.add_sink(sink_name, tx.clone());
    if let Err(error) = monitor.run() {
        send_or_eprint(
            Err(anyhow::anyhow!("PulseAudio mainloop failed: {:?}", error)),
            &mut tx,
        );
    }
}

fn send_block_data(
    context: Rc<RefCell<Context>>,
    sink_name: Option<String>,
    mut tx: Sender<Result<BlockData>>,
) {
    match sink_name {
        Some(sink_name) => {
            send_block_data_for_sink_name(context, sink_name, tx);
        }
        None => {
            let introspector = context.borrow().introspect();
            introspector.get_server_info(move |info| match &info.default_sink_name {
                Some(sink_name) => send_block_data_for_sink_name(
                    context.clone(),
                    sink_name.to_string(),
                    tx.clone(),
                ),
                None => send_or_eprint(
                    Err(anyhow::anyhow!("No default pulse audio sink found")),
                    &mut tx,
                ),
            });
        }
    }
}

fn send_block_data_for_sink_name(
    context: Rc<RefCell<Context>>,
    sink_name: String,
    mut tx: Sender<Result<BlockData>>,
) {
    let introspector = context.borrow().introspect();
    let sink_name_clone = sink_name.clone();
    let callback = move |list_result: ListResult<&SinkInfo>| {
        if let ListResult::Item(info) = list_result {
            let volume =
                (100.0 * info.volume.avg().0 as f64 / Volume::NORMAL.0 as f64).round() as u32;
            let data = BlockData {
                sink_name: sink_name_clone.clone(),
                muted: info.mute,
                volume,
            };
            send_or_eprint(Ok(data), &mut tx);
        }
    };
    introspector.get_sink_info_by_name(&sink_name, callback);
}
