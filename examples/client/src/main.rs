use tonic::transport::{Channel, Uri};
use utxorpc::spec::submit;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let uri: Uri = "http://[::1]:50052".parse()?;
    let channel = Channel::builder(uri).connect().await?;

    let mut client = submit::submit_service_client::SubmitServiceClient::new(channel);

    let tx = "84a300d9010281825820b6138021e8c128b5eec4747aac076ab401741a716e1c07928a75982471c9647800018182581d606e2e6d54e1a27ad640786852e20c22fb982ebcee4773a2926aae4b391b0000000129a7550c021a000292b1a100d9010281825820524e506f6a872c4ee3ee6d4b9913670c4b441860b3aa5438de92a676e20f527b58408723e9159dce8eca5ab525854437dd534f67d490e909d019287dac8509950f7cc4189cb86d5d6865464622f0a8f4909731aae1fdfb6e4826416df0932c6da709f5f6";

    let bytes = hex::decode(tx)?;

    let request = tonic::Request::new(submit::SubmitTxRequest {
        tx: vec![submit::AnyChainTx {
            r#type: Some(submit::any_chain_tx::Type::Raw(bytes.into())),
        }],
    });

    client.submit_tx(request).await?;

    Ok(())
}
