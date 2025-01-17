use anyhow::Result;

mod fanout;
mod ingest;
mod monitor;

#[derive(Debug)]
pub struct Transaction {}

pub async fn run() -> Result<()> {
    tokio::spawn(async {
        let ingest = ingest::Stage {};
        let fanout = fanout::Stage {};
        let monitor = monitor::Stage {};

        let policy: gasket::runtime::Policy = Default::default();

        let ingest = gasket::runtime::spawn_stage(ingest, policy.clone());
        let fanout = gasket::runtime::spawn_stage(fanout, policy.clone());
        let monitor = gasket::runtime::spawn_stage(monitor, policy.clone());

        let daemon = gasket::daemon::Daemon::new(vec![ingest, fanout, monitor]);
        daemon.block();
    })
    .await?;

    Ok(())
}
