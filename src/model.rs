use serde::Deserialize;

#[derive(Deserialize, Debug, PartialEq)]
pub struct Record {}

#[derive(Deserialize, Debug, PartialEq)]
pub enum Request {
    SignedTx(Record),
    UnsignedTx(Record),
    OracleMetadata(Record),
    AirDrop(Record),
}
