use gasket::runtime::Tether;
use serde::Deserialize;

use super::SourceOutputPort;

mod n2n;

pub enum Bootstrapper {
    N2N(n2n::Stage),
}

impl Bootstrapper {
    pub fn borrow_output(&mut self) -> &mut SourceOutputPort {
        match self {
            Bootstrapper::N2N(s) => &mut s.output,
        }
    }

    pub fn spawn(self, policy: gasket::runtime::Policy) -> Tether {
        match self {
            Bootstrapper::N2N(x) => gasket::runtime::spawn_stage(x, policy),
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum Config {
    N2N(n2n::Config),
}
impl Config {
    pub fn bootstrapper(self) -> Bootstrapper {
        match self {
            Config::N2N(c) => Bootstrapper::N2N(c.bootstrapper()),
        }
    }
}
