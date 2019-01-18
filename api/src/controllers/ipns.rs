use actix_web::HttpMessage;
use actix_web::HttpRequest;
use actix_web::HttpResponse;
use actix_web::Json;
use bigneon_db::prelude::*;
use chrono::prelude::*;
use chrono::Duration;
use db::Connection;
use errors::BigNeonError;
use globee::GlobeeIpnRequest;
use log::Level::Debug;

pub fn globee(
    (data, conn): (Json<GlobeeIpnRequest>, Connection),
) -> Result<HttpResponse, BigNeonError> {
    let data = data.into_inner();
    jlog!(Debug, "Globee IPN received", { "data": &data });
    let action = DomainAction::create(
        None,
        DomainActionTypes::PaymentProviderIPN,
        None,
        json!(data),
        None,
        None,
        Utc::now().naive_utc(),
        (Utc::now().naive_utc())
            .checked_add_signed(Duration::days(30))
            .unwrap(),
        5,
    )
    .commit(conn.get())?;

    Ok(HttpResponse::Ok().finish())
}
