use tonic::transport::{Channel, Uri};
use utxorpc::spec::submit;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let uri: Uri = "http://[::1]:50052".parse()?;
    let channel = Channel::builder(uri).connect().await?;

    let mut client = submit::submit_service_client::SubmitServiceClient::new(channel);

    let tx = "84a300d9010281825820b92468aa78e74719d856b16d5720a56914ccc08fbb460b7a3c1f9d308ebf1f1b00018182581d603f79e7eab3ab95c1f78824872ac6fd65f79d120868057f2bd19306f81b000000012a5ca17b021a000292b1a100d90102818258205d4b008e92a42846add4d060e49d7427700ced0ab8eb73e559acc14d228ca54758406542426eb508992f937ea4fb0ea5367d1423ce3e41c3f4d8de8462e2f53fca965428e3c8e77a072b5f96d6e52a3fdce0a2e517b672227b391e069d6545e62e06f5f6";

    let bytes = hex::decode(tx)?;

    let request = tonic::Request::new(submit::SubmitTxRequest {
        tx: vec![submit::AnyChainTx {
            r#type: Some(submit::any_chain_tx::Type::Raw(bytes.into())),
        }],
    });

    client.submit_tx(request).await?;

    Ok(())
}
