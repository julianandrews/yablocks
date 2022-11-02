use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

use anyhow::{Context as _, Result};
use futures::channel::mpsc::{Receiver, Sender};
use futures::{stream, StreamExt};
use pulse::callbacks::ListResult;
use pulse::context::subscribe::InterestMaskSet;
use pulse::context::{Context, FlagSet as ContextFlagSet};
use pulse::mainloop::standard::IterateResult;
use pulse::mainloop::standard::Mainloop;
use pulse::proplist::Proplist;
use pulse::volume::Volume;

use super::{BlockStream, BlockStreamConfig, Renderer};

#[derive(serde::Serialize, Debug, Clone)]
struct BlockData {
    sink_name: String,
    volume: u32,
    muted: bool,
}

struct Block {
    name: String,
    rx: Receiver<BlockData>,
    renderer: Renderer,
}

impl Block {
    fn new(
        name: String,
        template: String,
        rx: Receiver<BlockData>,
        renderer: Renderer,
    ) -> Result<Self> {
        renderer
            .lock()
            .unwrap()
            .register_template_string(&name, template)?;
        Ok(Self { name, rx, renderer })
    }

    async fn wait_for_output(&mut self) -> Result<Option<String>> {
        let data = match self.rx.next().await {
            Some(data) => data,
            None => return Ok(None),
        };
        let output = self.renderer.lock().unwrap().render(&self.name, &data)?;
        Ok(Some(output))
    }
}

impl BlockStreamConfig for crate::config::PulseVolumeConfig {
    fn to_stream(self, name: String, renderer: Renderer) -> Result<BlockStream> {
        let template = self.template.unwrap_or_else(|| "{{volume}}".to_string());
        let (tx, rx) = futures::channel::mpsc::channel::<BlockData>(1);
        let block = Block::new(name, template, rx, renderer)?;
        tokio::spawn(async move { monitor_sink(self.sink_name, tx).await });

        let stream = stream::unfold(block, move |mut block| async {
            let result = block.wait_for_output().await;
            let tagged_result = match result {
                Ok(output) => Ok((block.name.clone(), output?)),
                Err(error) => Err(error).with_context(|| format!("Error from {}", block.name)),
            };
            Some((tagged_result, block))
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

    fn add_sink(&mut self, sink_name: String, tx: Sender<BlockData>) {
        // Send the initial volume state
        send_block_data(self.context.clone(), sink_name.clone(), tx.clone());
        let context_clone = self.context.clone();
        self.context
            .borrow_mut()
            .set_subscribe_callback(Some(Box::new(move |_facility, _operation, _index| {
                send_block_data(context_clone.clone(), sink_name.clone(), tx.clone());
            })));
        self.context
            .borrow_mut()
            .subscribe(InterestMaskSet::SINK, |success| {
                if !success {
                    eprintln!("Failed to subscribe to pulse audio events");
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

fn send_block_data(context: Rc<RefCell<Context>>, sink_name: String, mut tx: Sender<BlockData>) {
    let introspector = context.borrow().introspect();
    introspector.get_sink_info_by_name(&sink_name.clone(), move |list_result| {
        if let ListResult::Item(info) = list_result {
            let volume =
                (100.0 * info.volume.avg().0 as f64 / Volume::NORMAL.0 as f64).round() as u32;
            let data = BlockData {
                sink_name: sink_name.clone(),
                muted: info.mute,
                volume,
            };
            while let Err(error) = tx.try_send(data.clone()) {
                if !error.is_full() {
                    eprintln!("Failed to send volume data: {:?}", error);
                    break;
                }
            }
        }
    });
}

async fn monitor_sink(sink_name: String, tx: Sender<BlockData>) -> Result<()> {
    let mut monitor = PulseVolumeMonitor::new()?;
    monitor.add_sink(sink_name, tx);
    monitor.run()
}
