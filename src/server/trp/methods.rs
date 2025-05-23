use std::sync::Arc;

use jsonrpsee::{
    core::client::ClientT,
    http_client::HttpClient,
    types::{ErrorCode, ErrorObject, ErrorObjectOwned, Params},
};
use serde::{Deserialize, Serialize};
use tracing::error;

use super::Context;

#[derive(Deserialize)]
pub enum Encoding {
    Hex,
    Base64,
}

#[derive(Deserialize)]
pub struct TrpSubmitRequest {
    pub encoding: Encoding,
    pub payload: String,
    pub version: String,
}

#[derive(Serialize)]
pub struct TrpSubmitResponse {
    pub hash: String,
}

pub async fn trp_submit(
    params: Params<'_>,
    _context: Arc<Context>,
) -> Result<serde_json::Value, ErrorObjectOwned> {
    let _request = params.parse::<TrpSubmitRequest>().map_err(|error| {
        error!(?error);
        ErrorObject::owned(
            ErrorCode::InvalidParams.code(),
            "invalid params",
            Some(error.to_string()),
        )
    })?;

    todo!()
}

pub async fn trp_resolve(
    params: Params<'_>,
    _context: Arc<Context>,
) -> Result<serde_json::Value, ErrorObjectOwned> {
    tracing::info!(method = "trp.resolve", "Received TRP request.");

    let client = HttpClient::builder()
        .build("http://localhost:8000")
        .map_err(|error| {
            error!(?error);
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Internal error",
                Some("Internal error"),
            )
        })?;

    let params = params.parse::<serde_json::Value>().map_err(|error| {
        error!(?error);
        ErrorObject::owned(
            ErrorCode::InvalidParams.code(),
            "invalid params",
            Some(error.to_string()),
        )
    });
    let params = jsonrpsee::core::rpc_params!(params);

    let response = client
        .request("trp.resolve", params)
        .await
        .map_err(|error| {
            error!(?error);
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "failed to resolve",
                Some(error.to_string()),
            )
        })?;

    Ok(response)
}

pub fn health(_context: &Context) -> bool {
    // TODO: add health check
    true
}
