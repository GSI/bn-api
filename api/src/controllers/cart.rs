use actix_web::State;
use actix_web::{http::StatusCode, HttpResponse};
use auth::user::User;
use bigneon_db::models::TicketType as Dbticket_types;
use bigneon_db::models::User as DbUser;
use bigneon_db::models::*;
use bigneon_db::utils::errors::Optional;
use communications::mailers;
use db::Connection;
use errors::BigNeonError;
use extractors::*;
use helpers::application;
use itertools::Itertools;
use payments::AuthThenCompletePaymentBehavior;
use payments::PaymentProcessor;
use payments::PaymentProcessorBehavior;
use payments::RedirectToPaymentPageBehavior;
use server::AppState;
use std::collections::HashMap;
use utils::ServiceLocator;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct CartItem {
    pub ticket_type_id: Uuid,
    pub quantity: u32,
    #[serde(default, deserialize_with = "deserialize_unless_blank")]
    pub redemption_code: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateCartRequest {
    pub items: Vec<CartItem>,
    pub box_office_pricing: Option<bool>,
}

pub fn update_cart(
    (connection, json, user): (Connection, Json<UpdateCartRequest>, User),
) -> Result<HttpResponse, BigNeonError> {
    let connection = connection.get();

    let box_office_pricing = json.box_office_pricing.unwrap_or(false);
    if box_office_pricing {
        let mut ticket_type_ids = json
            .items
            .iter()
            .map(|i| i.ticket_type_id)
            .collect::<Vec<Uuid>>();
        ticket_type_ids.sort();
        ticket_type_ids.dedup();

        for organization in Organization::find_by_ticket_type_ids(ticket_type_ids, connection)? {
            user.requires_scope_for_organization(
                Scopes::BoxOfficeTicketRead,
                &organization,
                connection,
            )?
        }
    }

    // Find the current cart of the user, if it exists.
    let mut cart = Order::find_or_create_cart(&user.user, connection)?;

    let order_items: Vec<UpdateOrderItem> = json
        .into_inner()
        .items
        .iter()
        .map(|i| UpdateOrderItem {
            quantity: i.quantity,
            ticket_type_id: i.ticket_type_id,
            redemption_code: i.redemption_code.clone(),
        })
        .collect();

    for order_item in &order_items {
        if !Dbticket_types::is_event_not_draft(&order_item.ticket_type_id, connection)? {
            return Ok(
                HttpResponse::BadRequest().json(json!({"error": "Event is invalid.".to_string()}))
            );
        }
    }
    cart.update_quantities(&order_items, box_office_pricing, false, connection)?;

    Ok(HttpResponse::Ok().json(Order::find(cart.id, connection)?.for_display(connection)?))
}

pub fn destroy((connection, user): (Connection, User)) -> Result<HttpResponse, BigNeonError> {
    let connection = connection.get();

    // Find the current cart of the user, if it exists.
    let mut cart = Order::find_or_create_cart(&user.user, connection)?;
    cart.update_quantities(&[], false, true, connection)?;

    Ok(HttpResponse::Ok().json(Order::find(cart.id, connection)?.for_display(connection)?))
}

pub fn replace_cart(
    (connection, json, user): (Connection, Json<UpdateCartRequest>, User),
) -> Result<HttpResponse, BigNeonError> {
    let connection = connection.get();

    let box_office_pricing = json.box_office_pricing.unwrap_or(false);
    if box_office_pricing {
        let mut ticket_type_ids = json
            .items
            .iter()
            .map(|i| i.ticket_type_id)
            .collect::<Vec<Uuid>>();
        ticket_type_ids.sort();
        ticket_type_ids.dedup();

        for organization in Organization::find_by_ticket_type_ids(ticket_type_ids, connection)? {
            user.requires_scope_for_organization(
                Scopes::BoxOfficeTicketRead,
                &organization,
                connection,
            )?
        }
    }

    // Find the current cart of the user, if it exists.
    let mut cart = Order::find_or_create_cart(&user.user, connection)?;

    let order_items: Vec<UpdateOrderItem> = json
        .into_inner()
        .items
        .iter()
        .map(|i| UpdateOrderItem {
            quantity: i.quantity,
            ticket_type_id: i.ticket_type_id,
            redemption_code: i.redemption_code.clone(),
        })
        .collect();

    for order_item in &order_items {
        if !Dbticket_types::is_event_not_draft(&order_item.ticket_type_id, connection)? {
            return Ok(
                HttpResponse::BadRequest().json(json!({"error": "Event is invalid.".to_string()}))
            );
        }
    }

    cart.update_quantities(&order_items, box_office_pricing, true, connection)?;

    Ok(HttpResponse::Ok().json(Order::find(cart.id, connection)?.for_display(connection)?))
}

pub fn show((connection, user): (Connection, User)) -> Result<HttpResponse, BigNeonError> {
    let connection = connection.get();
    let order = match Order::find_cart_for_user(user.id(), connection)? {
        Some(o) => o,
        None => return Ok(HttpResponse::Ok().json(json!({}))),
    };
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
        #[serde(default, deserialize_with = "deserialize_unless_blank")]
        reference: Option<String>,
        first_name: String,
        last_name: String,
        #[serde(default, deserialize_with = "deserialize_unless_blank")]
        email: Option<String>,
        #[serde(default, deserialize_with = "deserialize_unless_blank")]
        phone: Option<String>,
        #[serde(default, deserialize_with = "deserialize_unless_blank")]
        note: Option<String>,
    },
    Card {
        token: String,
        provider: String,
        save_payment_method: bool,
        set_default: bool,
    },
    PaymentMethod {
        #[serde(default, deserialize_with = "deserialize_unless_blank")]
        provider: Option<String>,
    },
    // Only for 0 amount carts
    Free,
}

pub fn checkout(
    (connection, json, user, state): (Connection, Json<CheckoutCartRequest>, User, State<AppState>),
) -> Result<HttpResponse, BigNeonError> {
    // TODO: Change application::unprocesable's in this method to validation errors.
    let req = json.into_inner();

    info!("CART: Checking out");
    let mut order = match Order::find_cart_for_user(user.id(), connection.get())? {
        Some(o) => o,
        None => return application::unprocessable("No cart exists for user"),
    };
    let order_id = order.id;
    order.lock_version(connection.get())?;

    let order_items = order.items(connection.get())?;

    //Assemble token ids and ticket instance ids for each asset in the order
    let mut tokens_per_asset: HashMap<Uuid, Vec<u64>> = HashMap::new();
    let mut wallet_id_per_asset: HashMap<Uuid, Uuid> = HashMap::new();
    for oi in &order_items {
        let tickets = TicketInstance::find_for_order_item(oi.id, connection.get())?;
        for ticket in &tickets {
            tokens_per_asset
                .entry(ticket.asset_id)
                .or_insert_with(|| Vec::new())
                .push(ticket.token_id as u64);
            wallet_id_per_asset
                .entry(ticket.asset_id)
                .or_insert(ticket.wallet_id);
        }
    }
    info!("CART: Verifying asset");
    //Just confirming that the asset is setup correctly before proceeding to payment.
    for asset_id in tokens_per_asset.keys() {
        let asset = Asset::find(*asset_id, connection.get())?;
        if asset.blockchain_asset_id.is_none() {
            return application::internal_server_error(
                "Could not complete this checkout because the asset has not been assigned on the blockchain",
            );
        }
    }

    let payment_response = match &req.method {
        PaymentRequest::Free => {
            info!("CART: Received checkout for free cart");
            if order.calculate_total(connection.get())? > 0 {
                // TODO: make this line cleaner
                return  application::unprocessable(
                    "Could not use free payment method this cart because it has a total greater than zero",
                );
            }
            checkout_free(&connection, order, &user)?
        }
        PaymentRequest::External {
            reference,
            first_name,
            last_name,
            email,
            phone,
            note,
        } => {
            info!("CART: Received external payment");
            checkout_external(
                &connection,
                order,
                reference.clone(),
                first_name.to_string(),
                last_name.to_string(),
                email.clone(),
                phone.clone(),
                note.clone(),
                &req,
                &user,
            )?
        }
        PaymentRequest::PaymentMethod { provider } => {
            info!("CART: Received provider payment");
            let provider = match provider {
                Some(provider) => provider.clone(),
                None => match user
                    .user
                    .default_payment_method(connection.get())
                    .optional()?
                {
                    Some(payment_method) => payment_method.name,
                    None => {
                        return application::unprocessable(
                            "Could not complete this cart because user has no default payment method",
                        );
                    }
                },
            };

            checkout_payment_processor(
                &connection,
                &mut order,
                None,
                &req,
                &user,
                &state.config.primary_currency,
                &provider,
                true,
                false,
                false,
                &state.service_locator,
            )?
        }
        PaymentRequest::Card {
            token,
            provider,
            save_payment_method,
            set_default,
        } => checkout_payment_processor(
            &connection,
            &mut order,
            Some(&token),
            &req,
            &user,
            &state.config.primary_currency,
            provider,
            false,
            *save_payment_method,
            *set_default,
            &state.service_locator,
        )?,
    };

    if payment_response.status() == StatusCode::OK {
        let conn = connection.get();
        let new_owner_wallet = Wallet::find_default_for_user(user.id(), conn)?;
        for (asset_id, token_ids) in &tokens_per_asset {
            let asset = Asset::find(*asset_id, conn)?;
            match asset.blockchain_asset_id {
                Some(a) => {
                    let wallet_id = match wallet_id_per_asset.get(asset_id) {
                        Some(w) => w.clone(),
                        None => return application::internal_server_error(
                            "Could not complete this checkout because wallet id not found for asset",
                        ),
                    };
                    let org_wallet = Wallet::find(wallet_id, conn)?;
                    state.config.tari_client.transfer_tokens(&org_wallet.secret_key, &org_wallet.public_key,
                                                             &a,
                                                             token_ids.clone(),
                                                             new_owner_wallet.public_key.clone(),
                    )?
                },
                None => return application::internal_server_error(
                    "Could not complete this checkout because the asset has not been assigned on the blockchain",
                ),
            }
        }

        let order = Order::find(order_id, conn)?;

        let display_order = order.for_display(conn)?;

        let user = DbUser::find(order.on_behalf_of_user_id.unwrap_or(order.user_id), conn)?;

        //Communicate purchase completed to user
        if let (Some(first_name), Some(email)) = (user.first_name, user.email) {
            mailers::cart::purchase_completed(
                &first_name,
                email,
                display_order,
                &state.config,
                conn,
            )?;
        }
    }

    Ok(payment_response)
}

fn checkout_free(
    conn: &Connection,
    order: Order,
    user: &User,
) -> Result<HttpResponse, BigNeonError> {
    let conn = conn.get();
    if order.status != OrderStatus::Draft {
        return application::unprocessable(
            "Could not complete this cart because it is not in the correct status",
        );
    }
    let mut order = order;
    order.add_external_payment(Some("Free Checkout".to_string()), user.id(), 0, conn)?;

    let order = Order::find(order.id, conn)?;
    Ok(HttpResponse::Ok().json(json!(order.for_display(conn)?)))
}

// TODO: This should actually probably move to an `orders` controller, since the
// user will not be calling this.
fn checkout_external(
    conn: &Connection,
    order: Order,
    reference: Option<String>,
    first_name: String,
    last_name: String,
    email: Option<String>,
    phone: Option<String>,
    note: Option<String>,
    checkout_request: &CheckoutCartRequest,
    user: &User,
) -> Result<HttpResponse, BigNeonError> {
    let conn = conn.get();

    // User must have external checkout permissions for all events in the cart.
    for (event_id, _) in &order.items(conn)?.into_iter().group_by(|oi| oi.event_id) {
        if let Some(event_id) = event_id {
            let organization = Organization::find_for_event(event_id, conn)?;
            user.requires_scope_for_organization(
                Scopes::OrderMakeExternalPayment,
                &organization,
                conn,
            )?;
        }
    }

    if order.status != OrderStatus::Draft {
        return application::unprocessable(
            "Could not complete this cart because it is not in the correct status",
        );
    }

    let mut guest: Option<DbUser> = None;

    if email.is_some() {
        guest = DbUser::find_by_email(email.as_ref().unwrap(), conn).optional()?;
    };
    if guest.is_none() {
        if phone.is_some() {
            guest = DbUser::find_by_phone(phone.as_ref().unwrap(), conn).optional()?;
        }
    }
    if guest.is_none() {
        guest = Some(DbUser::create_stub(
            first_name, last_name, email, phone, conn,
        )?);
    }

    let mut order = order.update(UpdateOrderAttributes { note: Some(note) }, conn)?;
    order.set_behalf_of_user(guest.unwrap(), user.id(), conn)?;

    order.add_external_payment(reference, user.id(), checkout_request.amount, conn)?;

    let order = Order::find(order.id, conn)?;
    Ok(HttpResponse::Ok().json(json!(order.for_display(conn)?)))
}

fn checkout_payment_processor(
    conn: &Connection,
    order: &mut Order,
    token: Option<&str>,
    req: &CheckoutCartRequest,
    auth_user: &User,
    currency: &str,
    provider_name: &str,
    use_stored_payment: bool,
    save_payment_method: bool,
    set_default: bool,
    service_locator: &ServiceLocator,
) -> Result<HttpResponse, BigNeonError> {
    info!("CART: Executing provider payment");
    let connection = conn.get();

    if order.user_id != auth_user.id() {
        return application::forbidden("This cart does not belong to you");
    } else if order.status != OrderStatus::Draft {
        return application::unprocessable(
            "Could not complete this cart because it is not in the correct status",
        );
    }

    let client = service_locator.create_payment_processor(provider_name)?;

    let token = if use_stored_payment {
        info!("CART: Using stored payment");
        match auth_user
            .user
            .payment_method(provider_name.to_string(), connection)
            .optional()?
        {
            Some(payment_method) => payment_method.provider,
            None => {
                return application::unprocessable(
                    "Could not complete this cart because stored provider does not exist",
                )
            }
        }
    } else {
        info!("CART: Not using stored payment");
        let token = match token {
            Some(t) => t,
            None => {
                return application::unprocessable(
                    "Could not complete this cart because no token provided",
                )
            }
        };

        if save_payment_method {
            info!("CART: User has requested to save the payment method");
            match auth_user
                .user
                .payment_method(provider_name.to_string(), connection)
                .optional()?
            {
                Some(payment_method) => {
                    let behavior = match client.behavior() {
                        PaymentProcessorBehavior::AuthThenComplete(b) => b,
                        _ =>  return application::unprocessable(
                            "Could not complete this cart using saved payment methods is not supported for this payment processor",
                        )
                    };
                    let client_response = behavior.update_repeat_token(
                        &payment_method.provider,
                        token,
                        "Big Neon something",
                    )?;
                    let payment_method_parameters = PaymentMethodEditableAttributes {
                        provider_data: Some(client_response.to_json()?),
                    };
                    payment_method.update(
                        &payment_method_parameters,
                        auth_user.id(),
                        connection,
                    )?;

                    payment_method.provider
                }
                None => {
                    let behavior = match client.behavior() {
                        PaymentProcessorBehavior::AuthThenComplete(b) => b,
                        _ =>  return application::unprocessable(
                            "Could not complete this cart using saved payment methods is not supported for this payment processor",
                        )
                    };
                    let repeat_token =
                        behavior.create_token_for_repeat_charges(token, "Big Neon")?;
                    let _payment_method = PaymentMethod::create(
                        auth_user.id(),
                        provider_name.to_string(),
                        set_default,
                        repeat_token.token.clone(),
                        repeat_token.to_json()?,
                    )
                    .commit(auth_user.id(), connection)?;
                    repeat_token.token
                }
            }
        } else {
            token.to_string()
        }
    };

    match client.behavior() {
        PaymentProcessorBehavior::AuthThenComplete(behavior) => auth_then_complete(
            &*behavior, token, req, currency, order, auth_user, conn, &*client,
        ),
        PaymentProcessorBehavior::RedirectToPaymentPage(behavior) => {
            redirect_to_payment_page(&*behavior, req, &auth_user.user)
        }
    }
}

fn auth_then_complete(
    client: &AuthThenCompletePaymentBehavior,
    token: String,
    req: &CheckoutCartRequest,
    currency: &str,
    order: &mut Order,
    auth_user: &User,
    conn: &Connection,
    payment_processor: &PaymentProcessor,
) -> Result<HttpResponse, BigNeonError> {
    let connection = conn.get();
    info!("CART: Auth'ing to payment provider");
    let auth_result = client.auth(
        &token,
        req.amount,
        currency,
        &format!("Big Neon"),
        vec![("order_id".to_string(), order.id.to_string())],
    )?;

    info!("CART: Saving payment to order");
    let payment = match order.add_credit_card_payment(
        auth_user.id(),
        req.amount,
        client.name(),
        auth_result.id.clone(),
        PaymentStatus::Authorized,
        auth_result.to_json()?,
        connection,
    ) {
        Ok(p) => p,
        Err(e) => {
            payment_processor.refund(&auth_result.id)?;
            return Err(e.into());
        }
    };

    conn.commit_transaction()?;
    conn.begin_transaction()?;

    info!("CART: Completing auth with payment provider");
    let charge_result = client.complete_authed_charge(&auth_result.id)?;
    info!("CART: Completing payment on order");
    info!("charge_result:{:?}", charge_result);
    match payment.mark_complete(charge_result.to_json()?, auth_user.id(), connection) {
        Ok(_) => {
            let order = Order::find(order.id, connection)?;
            Ok(HttpResponse::Ok().json(json!(order.for_display(connection)?)))
        }
        Err(e) => {
            payment_processor.refund(&auth_result.id)?;
            Err(e.into())
        }
    }
}

fn redirect_to_payment_page(
    client: &RedirectToPaymentPageBehavior,
    req: &CheckoutCartRequest,
    user: &DbUser,
) -> Result<HttpResponse, BigNeonError> {
    if user.email.is_none() {
        return application::unprocessable("User must have an email to check out");
    }
    let email = user.email.as_ref().unwrap().to_string();
    let response = client.create_payment_request(req.amount as f64 / 100_f64, email)?;
    Ok(HttpResponse::Ok().json(json!({"redirect_to": response.redirect_url})))
}
