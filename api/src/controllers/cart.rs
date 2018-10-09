use actix_web::Json;
use actix_web::State;
use actix_web::{http::StatusCode, HttpResponse};
use auth::user::User;
use bigneon_db::models::*;
use bigneon_db::utils::errors::Optional;
use db::Connection;
use errors::BigNeonError;
use helpers::application;
use payments::PaymentProcessor;
use server::AppState;
use std::collections::HashMap;
use utils::ServiceLocator;
//use tari_client::tari_messages::AssetInfoResult;
use uuid::Uuid;

#[derive(Serialize)]
pub struct CartResponse {
    pub cart_id: Uuid,
}

#[derive(Deserialize)]
pub struct AddToCartRequest {
    pub ticket_type_id: Uuid,
    pub quantity: i64,
}

pub fn add(
    (connection, json, user): (Connection, Json<AddToCartRequest>, User),
) -> Result<HttpResponse, BigNeonError> {
    let connection = connection.get();
    // Find the current cart of the user, if it exists.
    let current_cart = Order::find_cart_for_user(user.id(), connection).optional()?;

    // Create it if there isn't one
    let cart = if current_cart.is_none() {
        Order::create(user.id(), OrderTypes::Cart).commit(connection)?
    } else {
        current_cart.unwrap()
    };

    // Add the item
    cart.add_tickets(json.ticket_type_id, json.quantity, connection)?;

    Ok(HttpResponse::Created().json(&CartResponse { cart_id: cart.id }))
}

#[derive(Deserialize)]
pub struct RemoveCartRequest {
    pub cart_item_id: Uuid,
    pub quantity: Option<i64>,
}

pub fn remove(
    (connection, json, user): (Connection, Json<RemoveCartRequest>, User),
) -> Result<HttpResponse, BigNeonError> {
    let connection = connection.get();
    // Find the current cart of the user, if it exists.
    let current_cart = Order::find_cart_for_user(user.id(), connection).optional()?;

    match current_cart {
        Some(cart) => match cart.find_item(json.cart_item_id, connection).optional()? {
            Some(mut order_item) => {
                cart.remove_tickets(order_item, json.quantity, connection)?;
                Ok(HttpResponse::Ok().json(&CartResponse { cart_id: cart.id }))
            }
            None => application::unprocessable("Cart does not contain order item"),
        },
        None => application::unprocessable("No cart exists for user"),
    }
}

pub fn show((connection, user): (Connection, User)) -> Result<HttpResponse, BigNeonError> {
    let connection = connection.get();
    let order = Order::find_cart_for_user(user.id(), connection).optional()?;
    if order.is_none() {
        return Ok(HttpResponse::Ok().json(json!({})));
    }

    let order = order.unwrap();

    Ok(HttpResponse::Ok().json(order.for_display(connection)?))
}

#[derive(Deserialize)]
pub struct CheckoutCartRequest {
    pub amount: i64,
    pub method: PaymentRequest,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum PaymentRequest {
    External {
        reference: String,
    },
    Card {
        token: String,
        provider: String,
        save_payment_method: bool,
        set_default: bool,
    },
}

pub fn checkout(
    (connection, json, user, state): (Connection, Json<CheckoutCartRequest>, User, State<AppState>),
) -> Result<HttpResponse, BigNeonError> {
    let req = json.into_inner();

    let mut order = Order::find_cart_for_user(user.id(), connection.get())?;

    let order_items = order.items(connection.get())?;

    //Assemble token ids for each asset in the order
    let mut tokens_per_asset: HashMap<Uuid, Vec<u64>> = HashMap::new();
    for oi in &order_items {
        let tickets = TicketInstance::find_for_order_item(oi.id, connection.get())?;
        for ticket in &tickets {
            tokens_per_asset
                .entry(ticket.asset_id)
                .or_insert(vec![ticket.token_id as u64])
                .push(ticket.token_id as u64);
        }
    }
    //Just confirming that the asset is setup correctly before proceeding to payment.
    for (asset_id, _token_ids) in tokens_per_asset.iter() {
        let asset = Asset::find(asset_id, connection.get())?;
        if asset.blockchain_asset_id.is_none() {
            return application::internal_server_error(
                "Could not complete this checkout because the asset has not been assigned on the blockchain",
            );
        }
    }

    let payment_response = match &req.method {
        PaymentRequest::External { reference } => {
            checkout_external(&connection, &mut order, reference, &req, &user)
        }
        PaymentRequest::Card {
            token,
            provider,
            save_payment_method,
            set_default,
        } => checkout_payment_processor(
            &connection,
            &mut order,
            &token,
            &req,
            &user,
            &state.config.primary_currency,
            provider,
            *save_payment_method,
            *set_default,
            &state.service_locator,
        ),
    }?;

    if payment_response.status() == StatusCode::OK {
        for (asset_id, token_ids) in tokens_per_asset.iter() {
            let asset = Asset::find(asset_id, connection.get())?;
            match asset.blockchain_asset_id {
            Some(a) => state.config.tari_client.transfer_tokens(
                &a,
                token_ids.clone(),
                user.id().hyphenated().to_string(),
            )?,
            None => return application::internal_server_error(
                "Could not complete this checkout because the asset has not been assigned on the blockchain",
            ),
        }
        }
    }

    Ok(payment_response)
}

// TODO: This should actually probably move to an `orders` controller, since the
// user will not be calling this.
fn checkout_external(
    conn: &Connection,
    order: &mut Order,
    reference: &str,
    checkout_request: &CheckoutCartRequest,
    user: &User,
) -> Result<HttpResponse, BigNeonError> {
    let connection = conn.get();
    if !user.has_scope(Scopes::OrderMakeExternalPayment, None, connection)? {
        return application::unauthorized();
    }

    if order.status() != OrderStatus::Draft {
        return application::unprocessable(
            "Could not complete this cart because it is not in the correct status",
        );
    }

    let payment = order.add_external_payment(
        reference.to_string(),
        user.id(),
        checkout_request.amount,
        connection,
    )?;

    Ok(HttpResponse::Ok().json(json!({"payment_id": payment.id})))
}

fn checkout_payment_processor(
    conn: &Connection,
    order: &mut Order,
    token: &str,
    req: &CheckoutCartRequest,
    user: &User,
    currency: &str,
    provider_name: &str,
    save_payment_method: bool,
    set_default: bool,
    service_locator: &ServiceLocator,
) -> Result<HttpResponse, BigNeonError> {
    let connection = conn.get();

    if order.user_id != user.id() {
        return application::forbidden("This cart does not belong to you");
    }

    if order.status() != OrderStatus::Draft {
        return application::unprocessable(
            "Could not complete this cart because it is not in the correct status",
        );
    }

    if order.status() != OrderStatus::Draft {
        return application::unprocessable(
            "Could not complete this cart because it is not in the correct status",
        );
    }

    let client = service_locator.create_payment_processor(provider_name);

    let token = if save_payment_method {
        let repeat_token = client.create_token_for_repeat_charges(token, "Big Neon something")?;
        let _payment_method = PaymentMethod::create(
            user.id(),
            provider_name.to_string(),
            set_default,
            repeat_token.token.clone(),
            repeat_token.to_json()?,
        ).commit(connection)?;
        repeat_token.token.clone()
    } else {
        token.to_string()
    };

    let auth_result = client.auth(
        &token,
        req.amount,
        currency,
        "Tickets from Bigneon",
        vec![("order_id".to_string(), order.id.to_string())],
    )?;

    let payment = match order.add_credit_card_payment(
        user.id(),
        req.amount,
        provider_name.to_string(),
        auth_result.id.clone(),
        PaymentStatus::Authorized,
        auth_result.to_json()?,
        connection,
    ) {
        Ok(p) => p,
        Err(e) => {
            client.refund(&auth_result.id)?;
            return Err(e.into());
        }
    };

    conn.commit_transaction()?;
    conn.begin_transaction()?;

    let charge_result = client.complete_authed_charge(&auth_result.id)?;
    match payment.mark_complete(charge_result.to_json()?, connection) {
        Ok(_) => Ok(HttpResponse::Ok().json(json!({"payment_id": payment.id}))),
        Err(e) => {
            client.refund(&auth_result.id)?;
            Err(e.into())
        }
    }
}
