use tonic::transport::{Channel, Uri};
use utxorpc::spec::submit;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let uri: Uri = "http://[::1]:50052".parse()?;
    let channel = Channel::builder(uri).connect().await?;

    let mut client = submit::submit_service_client::SubmitServiceClient::new(channel);

    let tx = "84a300d9010282825820494b3e96e30b739dff2d5f88cd90078a063abe8a76e464d4020d3bf396c4639d00825820cf799ec1f2a176042dbe45f61c3b2a532bed0b1f69409b29f1ca668e3456692a00018282581d606e2e6d54e1a27ad640786852e20c22fb982ebcee4773a2926aae4b391b0000000218711a0082581d603f79e7eab3ab95c1f78824872ac6fd65f79d120868057f2bd19306f81a0080a847021a0002b149a100d90102818258205d4b008e92a42846add4d060e49d7427700ced0ab8eb73e559acc14d228ca5475840fc08fad217aff92eff577f0b7ac8c602f4bf2e58051ca0cdb4650ab1dc0446c548a82b29d1af866047311f311b3fd1352da45843e2a2d9177617b43da912840af5f6";

    let bytes = hex::decode(tx)?;

    let request = tonic::Request::new(submit::SubmitTxRequest {
        tx: vec![submit::AnyChainTx {
            r#type: Some(submit::any_chain_tx::Type::Raw(bytes.into())),
        }],
    });

    client.submit_tx(request).await?;

    Ok(())
}
