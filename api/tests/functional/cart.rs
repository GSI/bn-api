use actix_web::{http::StatusCode, HttpResponse};
use bigneon_api::controllers::cart;
use bigneon_api::controllers::cart::*;
use bigneon_api::extractors::*;
use bigneon_db::models::*;
use bigneon_db::schema::orders;
use chrono::prelude::*;
use chrono::Duration;
use diesel;
use diesel::prelude::*;
use functional::base;
use serde_json;
use support::database::TestDatabase;
use support::test_request::TestRequest;
use support::{self, *};

#[cfg(test)]
mod update_box_office_pricing_tests {
    use super::*;
    #[test]
    fn update_box_office_pricing_org_member() {
        base::cart::update_box_office_pricing(Roles::OrgMember, true);
    }
    #[test]
    fn update_box_office_pricing_admin() {
        base::cart::update_box_office_pricing(Roles::Admin, true);
    }
    #[test]
    fn update_box_office_pricing_user() {
        base::cart::update_box_office_pricing(Roles::User, false);
    }
    #[test]
    fn update_box_office_pricing_org_owner() {
        base::cart::update_box_office_pricing(Roles::OrgOwner, true);
    }
}

#[cfg(test)]
mod replace_box_office_pricing_tests {
    use super::*;
    #[test]
    fn replace_box_office_pricing_org_member() {
        base::cart::replace_box_office_pricing(Roles::OrgMember, true);
    }
    #[test]
    fn replace_box_office_pricing_admin() {
        base::cart::replace_box_office_pricing(Roles::Admin, true);
    }
    #[test]
    fn replace_box_office_pricing_user() {
        base::cart::replace_box_office_pricing(Roles::User, false);
    }
    #[test]
    fn replace_box_office_pricing_org_owner() {
        base::cart::replace_box_office_pricing(Roles::OrgOwner, true);
    }
}

#[test]
fn show() {
    let database = TestDatabase::new();
    let connection = database.connection.get();
    let user = database.create_user().finish();
    let cart = Order::find_or_create_cart(&user, connection).unwrap();

    let auth_user = support::create_auth_user_from_user(&user, Roles::User, None, &database);
    let response: HttpResponse = cart::show((database.connection.clone(), auth_user)).into();
    assert_eq!(response.status(), StatusCode::OK);
    let body = support::unwrap_body_to_string(&response).unwrap();
    let cart_response: DisplayOrder = serde_json::from_str(&body).unwrap();
    assert_eq!(cart.id, cart_response.id);
}

#[test]
fn show_no_cart() {
    let database = TestDatabase::new();
    let user = database.create_user().finish();
    let auth_user = support::create_auth_user_from_user(&user, Roles::User, None, &database);
    let response: HttpResponse = cart::show((database.connection, auth_user)).into();
    assert_eq!(response.status(), StatusCode::OK);
    let body = support::unwrap_body_to_string(&response).unwrap();
    assert_eq!(body, "{}");
}

#[test]
fn show_expired_cart() {
    let database = TestDatabase::new();
    let connection = database.connection.get();
    let user = database.create_user().finish();
    let cart = Order::find_or_create_cart(&user, connection).unwrap();
    let one_minute_ago = NaiveDateTime::from(Utc::now().naive_utc() - Duration::minutes(1));
    diesel::update(&cart)
        .set(orders::expires_at.eq(one_minute_ago))
        .get_result::<Order>(connection)
        .unwrap();

    let auth_user = support::create_auth_user_from_user(&user, Roles::User, None, &database);
    let response: HttpResponse = cart::show((database.connection.clone().into(), auth_user)).into();
    assert_eq!(response.status(), StatusCode::OK);
    let body = support::unwrap_body_to_string(&response).unwrap();
    assert_eq!(body, "{}");
}

#[test]
fn destroy() {
    let database = TestDatabase::new();
    let connection = database.connection.get();
    let event = database
        .create_event()
        .with_tickets()
        .with_ticket_pricing()
        .finish();

    let user = database.create_user().finish();
    let auth_user = support::create_auth_user_from_user(&user, Roles::User, None, &database);
    let ticket_type_id = event.ticket_types(connection).unwrap()[0].id;

    // Cart has existing items
    let mut cart = Order::find_or_create_cart(&user, connection).unwrap();
    cart.update_quantities(
        &vec![UpdateOrderItem {
            ticket_type_id: ticket_type_id,
            quantity: 10,
            redemption_code: None,
        }],
        false,
        false,
        connection,
    )
    .unwrap();
    let items = cart.items(&connection).unwrap();
    assert!(items.len() > 0);

    let response = cart::destroy((database.connection.clone().into(), auth_user)).unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Cart is cleared
    let cart = Order::find_or_create_cart(&user, connection).unwrap();
    assert!(cart.expires_at.is_none());
    let items = cart.items(&connection).unwrap();
    assert_eq!(0, items.len());
}

#[test]
fn destroy_without_existing_cart() {
    let database = TestDatabase::new();
    let connection = database.connection.get();
    let user = database.create_user().finish();
    let auth_user = support::create_auth_user_from_user(&user, Roles::User, None, &database);

    let response = cart::destroy((database.connection.clone().into(), auth_user)).unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Cart is cleared
    let cart = Order::find_cart_for_user(user.id, connection)
        .unwrap()
        .unwrap();
    assert!(cart.expires_at.is_none());
    let items = cart.items(&connection).unwrap();
    assert_eq!(0, items.len());
}

#[test]
fn update() {
    let database = TestDatabase::new();
    let connection = database.connection.get();
    let event = database
        .create_event()
        .with_tickets()
        .with_ticket_pricing()
        .finish();

    let user = database.create_user().finish();
    let ticket_type_id = event.ticket_types(connection).unwrap()[0].id;

    let input = Json(cart::UpdateCartRequest {
        box_office_pricing: None,
        items: vec![cart::CartItem {
            ticket_type_id,
            quantity: 2,
            redemption_code: None,
        }],
    });

    let auth_user = support::create_auth_user_from_user(&user, Roles::User, None, &database);
    let response =
        cart::update_cart((database.connection.clone().into(), input, auth_user)).unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let cart = Order::find_cart_for_user(user.id, &connection)
        .unwrap()
        .unwrap();
    let items = cart.items(&connection).unwrap();
    let order_item = items
        .iter()
        .find(|i| i.ticket_type_id == Some(ticket_type_id))
        .unwrap();

    let ticket_pricing =
        TicketPricing::find(order_item.ticket_pricing_id.unwrap(), connection).unwrap();
    assert_eq!(order_item.quantity, 2);
    let fee_item = order_item.find_fee_item(&connection).unwrap().unwrap();
    let fee_schedule_range =
        FeeScheduleRange::find(fee_item.fee_schedule_range_id.unwrap(), connection).unwrap();
    assert_eq!(
        fee_item.unit_price_in_cents,
        fee_schedule_range.fee_in_cents
    );
    assert_eq!(fee_item.quantity, 2);
    assert_eq!(
        order_item.unit_price_in_cents,
        ticket_pricing.price_in_cents
    );
}

#[test]
fn update_with_draft_event() {
    let database = TestDatabase::new();
    let connection = database.connection.get();
    let event = database
        .create_event()
        .with_status(EventStatus::Draft)
        .with_tickets()
        .with_ticket_pricing()
        .finish();

    let user = database.create_user().finish();
    let ticket_type_id = event.ticket_types(connection).unwrap()[0].id;

    let input = Json(cart::UpdateCartRequest {
        items: vec![cart::CartItem {
            ticket_type_id,
            quantity: 2,
            redemption_code: None,
        }],
        box_office_pricing: None,
    });

    let auth_user = support::create_auth_user_from_user(&user, Roles::User, None, &database);
    let response =
        cart::update_cart((database.connection.clone().into(), input, auth_user)).unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[test]
fn update_multiple() {
    let database = TestDatabase::new();
    let connection = database.connection.get();
    let event = database
        .create_event()
        .with_tickets()
        .with_ticket_type_count(2)
        .with_ticket_pricing()
        .finish();

    let user = database.create_user().finish();
    let ticket_types = event.ticket_types(connection).unwrap();
    let ticket_type_id = ticket_types[0].id;
    let ticket_type_id2 = ticket_types[1].id;
    let input = Json(cart::UpdateCartRequest {
        box_office_pricing: None,
        items: vec![
            cart::CartItem {
                ticket_type_id,
                quantity: 2,
                redemption_code: None,
            },
            cart::CartItem {
                ticket_type_id: ticket_type_id2,
                quantity: 3,
                redemption_code: None,
            },
        ],
    });
    let auth_user = support::create_auth_user_from_user(&user, Roles::User, None, &database);
    let response =
        cart::update_cart((database.connection.clone().into(), input, auth_user)).unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cart = Order::find_cart_for_user(user.id, connection)
        .unwrap()
        .unwrap();
    let cart_items = cart
        .items(&connection)
        .unwrap()
        .into_iter()
        .filter(|c| c.parent_id.is_none())
        .collect::<Vec<OrderItem>>();
    let order_item = &cart_items[0];
    let order_item2 = &cart_items[1];
    let ticket_pricing =
        TicketPricing::find(order_item.ticket_pricing_id.unwrap(), connection).unwrap();
    let ticket_pricing2 =
        TicketPricing::find(order_item2.ticket_pricing_id.unwrap(), connection).unwrap();
    assert_eq!(order_item.quantity, 2);
    assert_eq!(order_item2.quantity, 3);
    let fee_item = order_item.find_fee_item(connection).unwrap().unwrap();

    let fee_item2 = order_item2.find_fee_item(connection).unwrap().unwrap();

    let fee_schedule_range =
        FeeScheduleRange::find(fee_item.fee_schedule_range_id.unwrap(), connection).unwrap();

    let fee_schedule_range2 =
        FeeScheduleRange::find(fee_item2.fee_schedule_range_id.unwrap(), connection).unwrap();
    assert_eq!(
        fee_item.unit_price_in_cents,
        fee_schedule_range.fee_in_cents
    );
    assert_eq!(fee_item.quantity, 2);
    assert_eq!(
        order_item.unit_price_in_cents,
        ticket_pricing.price_in_cents
    );
    assert_eq!(
        fee_item2.unit_price_in_cents,
        fee_schedule_range2.fee_in_cents
    );
    assert_eq!(fee_item2.quantity, 3);
    assert_eq!(
        order_item2.unit_price_in_cents,
        ticket_pricing2.price_in_cents
    );
}

#[test]
fn add_with_increment() {
    let database = TestDatabase::new();
    let connection = database.connection.get();
    let event = database
        .create_event()
        .with_tickets()
        .with_ticket_pricing()
        .finish();

    let user = database.create_user().finish();
    let ticket_type = event.ticket_types(connection).unwrap().remove(0);
    let update_parameters = TicketTypeEditableAttributes {
        increment: Some(4),
        ..Default::default()
    };
    let ticket_type = ticket_type.update(update_parameters, connection).unwrap();
    let ticket_type_id = ticket_type.id;

    let input = Json(cart::UpdateCartRequest {
        box_office_pricing: None,
        items: vec![cart::CartItem {
            ticket_type_id,
            quantity: 4,
            redemption_code: None,
        }],
    });

    let auth_user = support::create_auth_user_from_user(&user, Roles::User, None, &database);
    let response = cart::update_cart((database.connection.clone(), input, auth_user)).unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let cart = Order::find_cart_for_user(user.id, connection)
        .unwrap()
        .unwrap();
    let order_item = cart
        .find_item_by_type(ticket_type_id, OrderItemTypes::Tickets, connection)
        .unwrap();
    let ticket_pricing =
        TicketPricing::find(order_item.ticket_pricing_id.unwrap(), connection).unwrap();
    assert_eq!(order_item.quantity, 4);
    let fee_item = order_item.find_fee_item(connection).unwrap().unwrap();
    let fee_schedule_range =
        FeeScheduleRange::find(fee_item.fee_schedule_range_id.unwrap(), connection).unwrap();
    assert_eq!(
        fee_item.unit_price_in_cents,
        fee_schedule_range.fee_in_cents
    );
    assert_eq!(fee_item.quantity, 4);
    assert_eq!(
        order_item.unit_price_in_cents,
        ticket_pricing.price_in_cents
    );
}

#[test]
fn update_with_increment_failure_invalid_quantity() {
    let database = TestDatabase::new();
    let connection = database.connection.get();
    let event = database
        .create_event()
        .with_tickets()
        .with_ticket_pricing()
        .finish();

    let user = database.create_user().finish();
    let ticket_type = event.ticket_types(connection).unwrap().remove(0);
    let update_parameters = TicketTypeEditableAttributes {
        increment: Some(4),
        ..Default::default()
    };
    let ticket_type = ticket_type.update(update_parameters, connection).unwrap();
    let ticket_type_id = ticket_type.id;

    let input = Json(cart::UpdateCartRequest {
        box_office_pricing: None,
        items: vec![cart::CartItem {
            ticket_type_id,
            quantity: 2,
            redemption_code: None,
        }],
    });

    let auth_user = support::create_auth_user_from_user(&user, Roles::User, None, &database);
    let response: HttpResponse =
        cart::update_cart((database.connection.clone().into(), input, auth_user)).into();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let validation_response = support::validation_response_from_response(&response).unwrap();
    let quantity = validation_response.fields.get("quantity").unwrap();
    assert_eq!(quantity[0].code, "quantity_invalid_increment");
    assert_eq!(
        &quantity[0].message.clone().unwrap().into_owned(),
        "Order item quantity invalid for ticket pricing increment"
    );
}

#[test]
fn update_with_existing_cart() {
    let database = TestDatabase::new();
    let connection = database.connection.get();
    let event = database
        .create_event()
        .with_tickets()
        .with_ticket_pricing()
        .finish();

    let user = database.create_user().finish();
    let ticket_type_id = event.ticket_types(connection).unwrap()[0].id;
    let cart = Order::find_or_create_cart(&user, connection).unwrap();

    let input = Json(cart::UpdateCartRequest {
        box_office_pricing: None,
        items: vec![cart::CartItem {
            ticket_type_id,
            quantity: 2,
            redemption_code: None,
        }],
    });

    let auth_user = support::create_auth_user_from_user(&user, Roles::User, None, &database);
    let response =
        cart::update_cart((database.connection.clone().into(), input, auth_user)).unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let items = cart.items(connection).unwrap();
    let order_item = items
        .iter()
        .find(|i| i.ticket_type_id == Some(ticket_type_id))
        .unwrap();

    let ticket_pricing =
        TicketPricing::find(order_item.ticket_pricing_id.unwrap(), connection).unwrap();
    assert_eq!(order_item.quantity, 2);
    let fee_item = order_item.find_fee_item(connection).unwrap().unwrap();
    let fee_schedule_range =
        FeeScheduleRange::find(fee_item.fee_schedule_range_id.unwrap(), connection).unwrap();
    assert_eq!(
        fee_item.unit_price_in_cents,
        fee_schedule_range.fee_in_cents
    );
    assert_eq!(fee_item.quantity, 2);
    assert_eq!(
        order_item.unit_price_in_cents,
        ticket_pricing.price_in_cents
    );
}

#[test]
fn reduce() {
    let database = TestDatabase::new();
    let connection = &database.connection.get();
    let event = database
        .create_event()
        .with_tickets()
        .with_ticket_pricing()
        .finish();
    let ticket_type_id = event.ticket_types(connection).unwrap()[0].id;
    let user = database.create_user().finish();
    let mut cart = Order::find_or_create_cart(&user, connection).unwrap();
    cart.update_quantities(
        &[UpdateOrderItem {
            ticket_type_id,
            quantity: 10,
            redemption_code: None,
        }],
        false,
        false,
        connection,
    )
    .unwrap();
    let items = cart.items(connection).unwrap();
    let order_item = items
        .iter()
        .find(|i| i.ticket_type_id == Some(ticket_type_id))
        .unwrap();
    let ticket_pricing =
        TicketPricing::find(order_item.ticket_pricing_id.unwrap(), connection).unwrap();
    assert_eq!(order_item.quantity, 10);
    let fee_item = order_item.find_fee_item(connection).unwrap().unwrap();
    let fee_schedule_range =
        FeeScheduleRange::find(fee_item.fee_schedule_range_id.unwrap(), connection).unwrap();
    assert_eq!(
        fee_item.unit_price_in_cents,
        fee_schedule_range.fee_in_cents
    );
    assert_eq!(fee_item.quantity, 10);
    assert_eq!(
        order_item.unit_price_in_cents,
        ticket_pricing.price_in_cents
    );

    let input = Json(cart::UpdateCartRequest {
        box_office_pricing: None,
        items: vec![cart::CartItem {
            ticket_type_id,
            quantity: 6,
            redemption_code: None,
        }],
    });

    let auth_user = support::create_auth_user_from_user(&user, Roles::User, None, &database);
    let response =
        cart::update_cart((database.connection.clone().into(), input, auth_user)).unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Contains additional item quantity so cart response still includes cart object
    let body = support::unwrap_body_to_string(&response).unwrap();
    let cart_response: DisplayOrder = serde_json::from_str(&body).unwrap();
    assert_eq!(cart.id, cart_response.id);
    let items = cart.items(connection).unwrap();
    let order_item = items
        .iter()
        .find(|i| i.ticket_type_id == Some(ticket_type_id))
        .unwrap();

    assert_eq!(order_item.quantity, 6);
    let fee_item = order_item.find_fee_item(connection).unwrap().unwrap();
    assert_eq!(
        fee_item.unit_price_in_cents,
        fee_schedule_range.fee_in_cents
    );
    assert_eq!(fee_item.quantity, 6);
    assert_eq!(
        order_item.unit_price_in_cents,
        ticket_pricing.price_in_cents
    );
}

#[test]
fn remove() {
    let database = TestDatabase::new();
    let connection = &database.connection.get();
    let event = database
        .create_event()
        .with_tickets()
        .with_ticket_pricing()
        .finish();
    let ticket_type_id = event.ticket_types(connection).unwrap()[0].id;
    let user = database.create_user().finish();
    let mut cart = Order::find_or_create_cart(&user, connection).unwrap();
    cart.update_quantities(
        &[UpdateOrderItem {
            ticket_type_id,
            quantity: 10,
            redemption_code: None,
        }],
        false,
        false,
        connection,
    )
    .unwrap();
    let items = cart.items(connection).unwrap();
    let order_item = items
        .iter()
        .find(|i| i.ticket_type_id == Some(ticket_type_id))
        .unwrap();

    let ticket_pricing =
        TicketPricing::find(order_item.ticket_pricing_id.unwrap(), connection).unwrap();
    assert_eq!(order_item.quantity, 10);
    let fee_item = order_item.find_fee_item(connection).unwrap().unwrap();
    let fee_schedule_range =
        FeeScheduleRange::find(fee_item.fee_schedule_range_id.unwrap(), connection).unwrap();
    assert_eq!(
        fee_item.unit_price_in_cents,
        fee_schedule_range.fee_in_cents
    );
    assert_eq!(fee_item.quantity, 10);
    assert_eq!(
        order_item.unit_price_in_cents,
        ticket_pricing.price_in_cents
    );

    let input = Json(cart::UpdateCartRequest {
        box_office_pricing: None,
        items: vec![cart::CartItem {
            ticket_type_id,
            quantity: 0,
            redemption_code: None,
        }],
    });

    let auth_user = support::create_auth_user_from_user(&user, Roles::User, None, &database);
    let response =
        cart::update_cart((database.connection.clone().into(), input, auth_user)).unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Contains additional item quantity so cart response still includes cart object
    let body = support::unwrap_body_to_string(&response).unwrap();
    let cart_response: DisplayOrder = serde_json::from_str(&body).unwrap();
    assert_eq!(cart.id, cart_response.id);

    let items = cart.items(connection).unwrap();

    assert_eq!(items.len(), 0);
}

#[test]
fn remove_with_increment() {
    let database = TestDatabase::new();
    let connection = &database.connection.get();
    let event = database
        .create_event()
        .with_tickets()
        .with_ticket_pricing()
        .finish();
    let ticket_type = event.ticket_types(connection).unwrap().remove(0);
    let update_parameters = TicketTypeEditableAttributes {
        increment: Some(4),
        ..Default::default()
    };
    let ticket_type = ticket_type.update(update_parameters, connection).unwrap();
    let ticket_type_id = ticket_type.id;
    let user = database.create_user().finish();
    let mut cart = Order::find_or_create_cart(&user, connection).unwrap();
    cart.update_quantities(
        &[UpdateOrderItem {
            ticket_type_id,
            quantity: 12,
            redemption_code: None,
        }],
        false,
        false,
        connection,
    )
    .unwrap();
    let items = cart.items(connection).unwrap();
    let order_item = items
        .iter()
        .find(|i| i.ticket_type_id == Some(ticket_type_id))
        .unwrap();

    let ticket_pricing =
        TicketPricing::find(order_item.ticket_pricing_id.unwrap(), connection).unwrap();
    assert_eq!(order_item.quantity, 12);
    let fee_item = order_item.find_fee_item(connection).unwrap().unwrap();
    let fee_schedule_range =
        FeeScheduleRange::find(fee_item.fee_schedule_range_id.unwrap(), connection).unwrap();
    assert_eq!(
        fee_item.unit_price_in_cents,
        fee_schedule_range.fee_in_cents
    );
    assert_eq!(fee_item.quantity, 12);
    assert_eq!(
        order_item.unit_price_in_cents,
        ticket_pricing.price_in_cents
    );

    let input = Json(cart::UpdateCartRequest {
        box_office_pricing: None,
        items: vec![cart::CartItem {
            ticket_type_id,
            quantity: 8,
            redemption_code: None,
        }],
    });

    let auth_user = support::create_auth_user_from_user(&user, Roles::User, None, &database);
    let response =
        cart::update_cart((database.connection.clone().into(), input, auth_user)).unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Contains additional item quantity so cart response still includes cart object
    let body = support::unwrap_body_to_string(&response).unwrap();
    let cart_response: DisplayOrder = serde_json::from_str(&body).unwrap();
    assert_eq!(cart.id, cart_response.id);
    let items = cart.items(connection).unwrap();
    let order_item = items
        .iter()
        .find(|i| i.ticket_type_id == Some(ticket_type_id))
        .unwrap();
    assert_eq!(order_item.quantity, 8);
    let fee_item = order_item.find_fee_item(connection).unwrap().unwrap();
    assert_eq!(
        fee_item.unit_price_in_cents,
        fee_schedule_range.fee_in_cents
    );
    assert_eq!(fee_item.quantity, 8);
    assert_eq!(
        order_item.unit_price_in_cents,
        ticket_pricing.price_in_cents
    );
}

#[test]
fn remove_with_increment_failure_invalid_quantity() {
    let database = TestDatabase::new();
    let connection = &database.connection.get();
    let event = database
        .create_event()
        .with_tickets()
        .with_ticket_pricing()
        .finish();
    let ticket_type = event.ticket_types(connection).unwrap().remove(0);
    let update_parameters = TicketTypeEditableAttributes {
        increment: Some(4),
        ..Default::default()
    };
    let ticket_type = ticket_type.update(update_parameters, connection).unwrap();
    let ticket_type_id = ticket_type.id;
    let user = database.create_user().finish();
    let mut cart = Order::find_or_create_cart(&user, connection).unwrap();
    cart.update_quantities(
        &[UpdateOrderItem {
            ticket_type_id,
            quantity: 12,
            redemption_code: None,
        }],
        false,
        false,
        connection,
    )
    .unwrap();
    let items = cart.items(connection).unwrap();
    let order_item = items
        .iter()
        .find(|i| i.ticket_type_id == Some(ticket_type_id))
        .unwrap();

    let ticket_pricing =
        TicketPricing::find(order_item.ticket_pricing_id.unwrap(), connection).unwrap();
    assert_eq!(order_item.quantity, 12);
    let fee_item = order_item.find_fee_item(connection).unwrap().unwrap();
    let fee_schedule_range =
        FeeScheduleRange::find(fee_item.fee_schedule_range_id.unwrap(), connection).unwrap();
    assert_eq!(
        fee_item.unit_price_in_cents,
        fee_schedule_range.fee_in_cents
    );
    assert_eq!(fee_item.quantity, 12);
    assert_eq!(
        order_item.unit_price_in_cents,
        ticket_pricing.price_in_cents
    );

    let input = Json(cart::UpdateCartRequest {
        box_office_pricing: None,
        items: vec![cart::CartItem {
            ticket_type_id,
            quantity: 5,
            redemption_code: None,
        }],
    });

    let auth_user = support::create_auth_user_from_user(&user, Roles::User, None, &database);
    let response: HttpResponse =
        cart::update_cart((database.connection.clone().into(), input, auth_user)).into();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let validation_response = support::validation_response_from_response(&response).unwrap();
    let quantity = validation_response.fields.get("quantity").unwrap();
    assert_eq!(quantity[0].code, "quantity_invalid_increment");
    assert_eq!(
        &quantity[0].message.clone().unwrap().into_owned(),
        "Order item quantity invalid for ticket pricing increment"
    );
}

#[test]
fn checkout_external() {
    let database = TestDatabase::new();
    let event = database
        .create_event()
        .with_tickets()
        .with_ticket_pricing()
        .finish();

    let user = database.create_user().finish();

    let _order = database
        .create_cart()
        .for_user(&user)
        .for_event(&event)
        .finish();
    let request = TestRequest::create();

    let input = Json(cart::CheckoutCartRequest {
        amount: 100,
        method: PaymentRequest::External {
            reference: Some("TestRef".to_string()),
            first_name: "First".to_string(),
            last_name: "Last".to_string(),
            email: Some("easdf@test.com".to_string()),
            phone: None,
            note: None,
        },
    });

    // Must be admin to check out external
    let user = support::create_auth_user_from_user(&user, Roles::Admin, None, &database);

    let response = cart::checkout((
        database.connection.into(),
        input,
        user,
        request.extract_state(),
    ))
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[test]
fn checkout_free() {
    let database = TestDatabase::new();
    let connection = database.connection.get();
    let event = database
        .create_event()
        .with_tickets()
        .with_ticket_pricing()
        .finish();

    let user = database.create_user().finish();

    let order = database
        .create_cart()
        .with_free_items()
        .for_user(&user)
        .for_event(&event)
        .finish();
    let request = TestRequest::create();

    let input = Json(cart::CheckoutCartRequest {
        amount: 0,
        method: PaymentRequest::Free,
    });

    let user = support::create_auth_user_from_user(&user, Roles::User, None, &database);

    let response = cart::checkout((
        database.connection.clone().into(),
        input,
        user,
        request.extract_state(),
    ))
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Reload order
    let order = Order::find(order.id, connection).unwrap();
    assert_eq!(order.status, OrderStatus::Paid);
}

#[test]
fn checkout_free_for_paid_items() {
    let database = TestDatabase::new();
    let connection = database.connection.get();
    let event = database
        .create_event()
        .with_tickets()
        .with_ticket_pricing()
        .finish();

    let user = database.create_user().finish();

    let order = database
        .create_cart()
        .for_user(&user)
        .for_event(&event)
        .finish();
    let request = TestRequest::create();

    let input = Json(cart::CheckoutCartRequest {
        amount: 0,
        method: PaymentRequest::Free,
    });

    let user = support::create_auth_user_from_user(&user, Roles::User, None, &database);

    let response: HttpResponse = cart::checkout((
        database.connection.clone().into(),
        input,
        user,
        request.extract_state(),
    ))
    .into();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let expected_json = HttpResponse::new(StatusCode::UNPROCESSABLE_ENTITY).into_builder().json(json!({
        "error": "Could not use free payment method this cart because it has a total greater than zero"
    }));
    let expected_text = unwrap_body_to_string(&expected_json).unwrap();
    let body = unwrap_body_to_string(&response).unwrap();
    assert_eq!(body, expected_text);

    // Reload order
    let order = Order::find(order.id, connection).unwrap();
    assert_eq!(order.status, OrderStatus::Draft);
}
