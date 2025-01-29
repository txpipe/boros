use tonic::transport::{Channel, Uri};
use utxorpc::spec::submit;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let uri: Uri = "http://[::1]:50052".parse()?;
    let channel = Channel::builder(uri).connect().await?;

    let mut client = submit::submit_service_client::SubmitServiceClient::new(channel);

    let tx = "84a300d9010281825820451a62d8cd4c59bf24e09f333d4edce36610285a9af20de2fb0edacf6f2d15b600018182581d606e2e6d54e1a27ad640786852e20c22fb982ebcee4773a2926aae4b391b00000001a10ea803021a000292b1a100d9010281825820524e506f6a872c4ee3ee6d4b9913670c4b441860b3aa5438de92a676e20f527b58402683a6a77af236bc0e2197a45841afc258d89df2ab5732fa040e977fb2c22255b9d9cc35d33153e87d978cb187e399d0c4071fbf95d33dc380677219450e0908f5f6";

    let bytes = hex::decode(tx)?;

    let request = tonic::Request::new(submit::SubmitTxRequest {
        tx: vec![submit::AnyChainTx {
            r#type: Some(submit::any_chain_tx::Type::Raw(bytes.into())),
        }],
    });

    client.submit_tx(request).await?;

    Ok(())
}
