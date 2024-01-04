use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use enstate_shared::models::profile::Profile;
use ethers_core::types::Address;
use futures::future::try_join_all;
use serde::Deserialize;

use crate::models::bulk::BulkResponse;
use crate::routes::{
    http_simple_status_error, profile_http_error_mapper, validate_bulk_input, FreshQuery, Qs,
    RouteError,
};

#[utoipa::path(
    get,
    path = "/a/{address}",
    responses(
        (status = 200, description = "Successfully found address.", body = ENSProfile),
        (status = BAD_REQUEST, description = "Invalid address.", body = ErrorResponse),
        (status = NOT_FOUND, description = "No name was associated with this address.", body = ErrorResponse),
        (status = UNPROCESSABLE_ENTITY, description = "Reverse record not owned by this address.", body = ErrorResponse),
    ),
    params(
        ("address" = String, Path, description = "Address to lookup name data for"),
    )
)]
pub async fn get(
    Path(address): Path<String>,
    Query(query): Query<FreshQuery>,
    State(state): State<Arc<crate::AppState>>,
) -> Result<Json<Profile>, RouteError> {
    get_bulk(
        Qs(AddressGetBulkQuery {
            fresh: query,
            addresses: vec![address],
        }),
        State(state),
    )
    .await
    .map(|res| Json(res.0.response.get(0).expect("index 0 should exist").clone()))
}

#[derive(Deserialize)]
pub struct AddressGetBulkQuery {
    // TODO (@antony1060): remove when proper serde error handling
    #[serde(default)]
    addresses: Vec<String>,

    #[serde(flatten)]
    fresh: FreshQuery,
}

#[utoipa::path(
    get,
    path = "/bulk/a/",
    responses(
        (status = 200, description = "Successfully found address.", body = BulkResponse<ENSProfile>),
        (status = BAD_REQUEST, description = "Invalid address.", body = ErrorResponse),
        (status = NOT_FOUND, description = "No name was associated with this address.", body = ErrorResponse),
        (status = UNPROCESSABLE_ENTITY, description = "Reverse record not owned by this address.", body = ErrorResponse),
    ),
    params(
        ("addresses" = Vec<String>, Path, description = "Addresses to lookup name data for"),
    )
)]
pub async fn get_bulk(
    Qs(query): Qs<AddressGetBulkQuery>,
    State(state): State<Arc<crate::AppState>>,
) -> Result<Json<BulkResponse<Profile>>, RouteError> {
    let addresses = validate_bulk_input(&query.addresses, 10)?;

    let addresses = addresses
        .iter()
        .map(|address| address.parse::<Address>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| http_simple_status_error(StatusCode::BAD_REQUEST))?;

    let profiles = addresses
        .iter()
        .map(|address| {
            state
                .service
                .resolve_from_address(*address, query.fresh.fresh)
        })
        .collect::<Vec<_>>();

    let joined = try_join_all(profiles)
        .await
        .map_err(profile_http_error_mapper)?;

    Ok(Json(joined.into()))
}
