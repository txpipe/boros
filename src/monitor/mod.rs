use anyhow::Result;
use gasket::{
    messaging::tokio::ChannelRecvAdapter,
    runtime::{Policy, Tether},
};
use serde::{Deserialize, Serialize};

mod webhook;

#[derive(Debug, Serialize, Clone)]
pub struct Event {}

pub type HookInputPort = gasket::messaging::InputPort<Event>;

pub enum Bootstrapper {
    Webhook(webhook::Stage),
}

impl Bootstrapper {
    pub fn borrow_input(&mut self) -> &mut HookInputPort {
        match self {
            Bootstrapper::Webhook(x) => &mut x.input,
        }
    }

    pub fn spawn(self, policy: gasket::runtime::Policy) -> Tether {
        match self {
            Bootstrapper::Webhook(x) => gasket::runtime::spawn_stage(x, policy),
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum Config {
    Webhook(webhook::Config),
}
impl Config {
    pub fn bootstrapper(self) -> Bootstrapper {
        match self {
            Config::Webhook(c) => Bootstrapper::Webhook(c.bootstrapper()),
        }
    }
}

pub async fn run(config: Config, receiver: ChannelRecvAdapter<Event>) -> Result<()> {
    tokio::spawn(async {
        let mut hook = config.bootstrapper();
        hook.borrow_input().connect(receiver);
        let hook = hook.spawn(Policy::default());

        let daemon = gasket::daemon::Daemon::new(vec![hook]);
        daemon.block();
    })
    .await?;

    Ok(())
}
