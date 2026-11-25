use std::{net::SocketAddr, time::Duration};

use reqwest::{Client, Response, StatusCode};
use serde_json::Value;

pub fn client() -> Result<Client, &'static str> {
    Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|_| "could not build local HTTP client")
}

pub async fn get(
    client: &Client,
    address: SocketAddr,
    path: &str,
    query: &[(String, String)],
) -> Result<Value, &'static str> {
    if address.port() == 0 {
        return Err("supply the actual query port printed by the running collector");
    }
    let response = client
        .get(format!("http://{address}{path}"))
        .query(query)
        .send()
        .await
        .map_err(|_| "could not reach local query API; run request-xray dev")?;
    match response.status() {
        StatusCode::OK => read_json(response).await,
        StatusCode::NOT_FOUND => Err("trace not found"),
        StatusCode::BAD_REQUEST => Err("invalid query filters or cursor"),
        StatusCode::PAYLOAD_TOO_LARGE => Err("trace page too large; reduce --limit"),
        _ => Err("local query API unavailable"),
    }
}

pub async fn export(
    client: &Client,
    address: SocketAddr,
    body: &'static [u8],
) -> Result<(), &'static str> {
    if address.port() == 0 {
        return Err("supply the actual OTLP port printed by the running collector");
    }
    let response = client
        .post(format!("http://{address}/v1/traces"))
        .header("content-type", "application/x-protobuf")
        .body(body)
        .send()
        .await
        .map_err(|_| "could not reach local OTLP listener")?;
    // Full OTLP protobuf success has an empty body.
    if response.status() != StatusCode::OK
        || response
            .headers()
            .get("content-type")
            .is_none_or(|media| media != "application/x-protobuf")
    {
        return Err("local OTLP listener did not acknowledge the export");
    }
    let mut response = response;
    while let Some(bytes) = response
        .chunk()
        .await
        .map_err(|_| "could not read OTLP acknowledgment")?
    {
        if !bytes.is_empty() {
            return Err("local OTLP listener reported partial success");
        }
    }
    Ok(())
}

async fn read_json(mut response: Response) -> Result<Value, &'static str> {
    let mut output = Vec::new();
    while let Some(bytes) = response
        .chunk()
        .await
        .map_err(|_| "could not read local API response")?
    {
        if output.len().saturating_add(bytes.len()) > 16 * 1024 * 1024 {
            return Err("local API response exceeds 16 MiB");
        }
        output.extend_from_slice(&bytes);
    }
    serde_json::from_slice(&output).map_err(|_| "invalid local API response")
}
