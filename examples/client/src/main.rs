use tonic::transport::{Channel, Uri};
use utxorpc::spec::submit;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let uri: Uri = "http://[::1]:50052".parse()?;
    let channel = Channel::builder(uri).connect().await?;

    let mut client = submit::submit_service_client::SubmitServiceClient::new(channel);

    let tx = "84a300d90102818258206ddb32f9ffe54e4eb6f4df9887dbec5f42615998b4bb401f71fed439069f219a00018182581d606e2e6d54e1a27ad640786852e20c22fb982ebcee4773a2926aae4b391b0000000129a4c25b021a000292b1a100d9010281825820524e506f6a872c4ee3ee6d4b9913670c4b441860b3aa5438de92a676e20f527b5840ceb8b8a24cb77ab981ffd5706a77e8ff1a579ecbc27359def46ed5b157418d7643f3b255ae1da4c223690f038f644181e16266d4961c1d4c94d1bf36cdd6de03f5f6";

    let bytes = hex::decode(tx)?;

    let request = tonic::Request::new(submit::SubmitTxRequest {
        tx: vec![submit::AnyChainTx {
            r#type: Some(submit::any_chain_tx::Type::Raw(bytes.into())),
        }],
    });

    client.submit_tx(request).await?;

    Ok(())
}
