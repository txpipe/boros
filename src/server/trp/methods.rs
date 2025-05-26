use std::sync::Arc;

use base64::{prelude::BASE64_STANDARD, Engine};
use jsonrpsee::{
    core::{client::ClientT, params::ObjectParams},
    http_client::HttpClient,
    types::{ErrorCode, ErrorObject, ErrorObjectOwned, Params},
};
use pallas::ledger::traverse::MultiEraTx;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{
    storage::Transaction,
    validation::{evaluate_tx, validate_tx},
};

use super::Context;

#[derive(Deserialize)]
pub enum Encoding {
    #[serde(rename = "hex")]
    Hex,
    #[serde(rename = "base64")]
    Base64,
}

#[derive(Deserialize)]
pub struct TrpSubmitTxRequest {
    pub encoding: Encoding,
    pub payload: String,
}

#[derive(Deserialize)]
pub struct TrpSubmitRequest {
    pub tx: TrpSubmitTxRequest,
}

#[derive(Serialize)]
pub struct TrpSubmitResponse {
    pub hash: String,
}

pub async fn trp_resolve(
    params: Params<'_>,
    context: Arc<Context>,
) -> Result<serde_json::Value, ErrorObjectOwned> {
    tracing::info!(method = "trp.resolve", "Received TRP request.");

    let mut client_builder = HttpClient::builder();

    if let Some(headers) = &context.config.server.headers {
        let headermap = headers.try_into().unwrap();
        client_builder = client_builder.set_headers(headermap);
    }

    let client = client_builder
        .build(&context.config.server.uri)
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
    })?;

    let object_params = match params {
        serde_json::Value::Object(map) => {
            let mut obj = ObjectParams::new();
            map.into_iter()
                .for_each(|(key, value)| obj.insert(&key, value).unwrap());
            Ok(obj)
        }
        _ => {
            error!("invalid params");
            Err(ErrorObject::owned(
                ErrorCode::InvalidParams.code(),
                "invalid params",
                Some("params must be a json"),
            ))
        }
    }?;

    let response = client
        .request("trp.resolve", object_params)
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

pub async fn trp_submit(
    params: Params<'_>,
    context: Arc<Context>,
) -> Result<serde_json::Value, ErrorObjectOwned> {
    tracing::info!(method = "trp.submit", "Received TRP request.");

    let request = params.parse::<TrpSubmitRequest>().map_err(|error| {
        error!(?error);
        ErrorObject::owned(
            ErrorCode::InvalidParams.code(),
            "invalid params",
            Some(error.to_string()),
        )
    })?;

    let raw = match request.tx.encoding {
        Encoding::Hex => hex::decode(request.tx.payload).map_err(|error| {
            error!(?error);
            ErrorObject::owned(
                ErrorCode::ParseError.code(),
                "invalid tx hex encoding",
                Some(error.to_string()),
            )
        })?,
        Encoding::Base64 => BASE64_STANDARD
            .decode(request.tx.payload)
            .map_err(|error| {
                error!(?error);
                ErrorObject::owned(
                    ErrorCode::ParseError.code(),
                    "invalid tx base64 encoding",
                    Some(error.to_string()),
                )
            })?,
    };

    let metx = MultiEraTx::decode(&raw).map_err(|error| {
        error!(?error);
        ErrorObject::owned(
            ErrorCode::InvalidParams.code(),
            "invalid tx",
            Some(error.to_string()),
        )
    })?;

    let hash = metx.hash();

    if let Err(error) = validate_tx(&metx, context.u5c_adapter.clone()).await {
        error!(?error);
        return Err(ErrorObject::owned(
            ErrorCode::InvalidRequest.code(),
            "failed to submit tx",
            Some(error.to_string()),
        ));
    }

    if let Err(error) = evaluate_tx(&metx, context.u5c_adapter.clone()).await {
        error!(?error);
        return Err(ErrorObject::owned(
            ErrorCode::InvalidRequest.code(),
            "failed to submit tx",
            Some(error.to_string()),
        ));
    }

    let tx_storage = Transaction::new(hash.to_string(), raw.to_vec());

    context
        .tx_storage
        .create(&vec![tx_storage])
        .await
        .map_err(|error| {
            error!(?error);
            ErrorObject::owned(
                ErrorCode::InvalidRequest.code(),
                "internal error",
                Some(error.to_string()),
            )
        })?;

    let hash = hex::encode(hash);
    info!(?hash, "submitting tx");

    let response = serde_json::to_value(TrpSubmitResponse { hash }).map_err(|error| {
        error!(?error);
        ErrorObject::owned(
            ErrorCode::InternalError.code(),
            "transaction accepted, but error to encode response",
            Some(error.to_string()),
        )
    })?;

    Ok(response)
}

pub async fn health(context: Arc<Context>) -> Result<serde_json::Value, ErrorObjectOwned> {
    tracing::info!(method = "health", "Received TRP request.");

    let mut client_builder = HttpClient::builder();

    if let Some(headers) = &context.config.server.headers {
        let headermap = headers.try_into().unwrap();
        client_builder = client_builder.set_headers(headermap);
    }

    let client = client_builder
        .build(&context.config.server.uri)
        .map_err(|error| {
            error!(?error);
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Internal error",
                Some("Internal error"),
            )
        })?;

    let response = client
        .request("health", ObjectParams::default())
        .await
        .map_err(|error| {
            error!(?error);
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "failed health check",
                Some(error.to_string()),
            )
        })?;

    Ok(response)
}
