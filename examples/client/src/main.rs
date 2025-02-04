use tonic::transport::{Channel, Uri};
use utxorpc::spec::submit;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let uri: Uri = "http://[::1]:50052".parse()?;
    let channel = Channel::builder(uri).connect().await?;

    let mut client = submit::submit_service_client::SubmitServiceClient::new(channel);

    let tx = "84a300d9010281825820a2aafdbbfd255fa1ca2372c24b8e1021a8c7bc20360c23d8e1d5ba1de0ccffc400018182581d603f79e7eab3ab95c1f78824872ac6fd65f79d120868057f2bd19306f81b000000012a61c6dd021a000292b1a100d90102818258205d4b008e92a42846add4d060e49d7427700ced0ab8eb73e559acc14d228ca5475840369982092dc0eba4d873bba58169334063d945cceca0616f83980889dacb6ccb18406ff6f7b17fa04cee51de26517946c015486ba51e5ab4ea39b0ede3b92a05f5f6";

    let bytes = hex::decode(tx)?;

    let request = tonic::Request::new(submit::SubmitTxRequest {
        tx: vec![submit::AnyChainTx {
            r#type: Some(submit::any_chain_tx::Type::Raw(bytes.into())),
        }],
    });

    client.submit_tx(request).await?;

    Ok(())
}
