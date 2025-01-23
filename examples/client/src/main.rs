use tonic::transport::{Channel, Uri};
use utxorpc::spec::submit;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let uri: Uri = "http://[::1]:50052".parse()?;
    let channel = Channel::builder(uri).connect().await?;

    let mut client = submit::submit_service_client::SubmitServiceClient::new(channel);

    let tx = "84a300d9010281825820cdc219e7abe938a35ca074d4bd02d6ccc3c2fc25d1462af07b6c1e8f40933af200018282581d603f79e7eab3ab95c1f78824872ac6fd65f79d120868057f2bd19306f81a3b9aca0082581d603f79e7eab3ab95c1f78824872ac6fd65f79d120868057f2bd19306f81a77c0bd2f021a0002990da100d90102818258205d4b008e92a42846add4d060e49d7427700ced0ab8eb73e559acc14d228ca5475840f3f12cbfd551e5e51f9eb32fcf695c3a63ec3dfb7329108f45b441cafc7a706659d06238665327779e32415c91b6190e0cd00096aee41f6e405be59d69462708f5f6";

    let bytes = hex::decode(tx)?;

    let request = tonic::Request::new(submit::SubmitTxRequest {
        tx: vec![submit::AnyChainTx {
            r#type: Some(submit::any_chain_tx::Type::Raw(bytes.into())),
        }],
    });

    client.submit_tx(request).await?;

    Ok(())
}
