use anyhow::Result;
use gasket::messaging::tokio::ChannelSendAdapter;
use serde::Deserialize;

use crate::monitor;

mod confirmation;
mod parse;
mod sources;

#[derive(Debug, Clone)]
pub enum Event {
    CborBlock(Vec<u8>),
}

pub type SourceOutputPort = gasket::messaging::OutputPort<Event>;
pub type ParseInputPort = gasket::messaging::InputPort<Event>;
pub type ParseOutputPort = gasket::messaging::OutputPort<Event>;
pub type ConfirmationInputPort = gasket::messaging::InputPort<Event>;

#[derive(Deserialize)]
pub struct Config {
    source: sources::Config,
}

pub async fn run(config: Config, monitor_sender: ChannelSendAdapter<monitor::Event>) -> Result<()> {
    tokio::spawn(async {
        let mut source = config.source.bootstrapper();

        let mut parse = parse::Stage {
            input: Default::default(),
            output: Default::default(),
        };
        gasket::messaging::tokio::connect_ports(&mut source.borrow_output(), &mut parse.input, 100);

        let mut confirmation = confirmation::Stage {
            input: Default::default(),
            monitor: monitor_sender,
        };
        gasket::messaging::tokio::connect_ports(&mut parse.output, &mut confirmation.input, 100);

        let policy: gasket::runtime::Policy = Default::default();

        let source = source.spawn(policy.clone());
        let parse = gasket::runtime::spawn_stage(parse, policy.clone());
        let confirmation = gasket::runtime::spawn_stage(confirmation, policy.clone());

        let daemon = gasket::daemon::Daemon::new(vec![source, parse, confirmation]);
        daemon.block();
    })
    .await?;

    Ok(())
}
