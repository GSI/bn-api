use bigneon_db::dev::TestProject;
use bigneon_db::prelude::*;
use chrono::prelude::*;
use chrono::NaiveDateTime;
use diesel;
use diesel::result::Error;
use diesel::sql_types;
use diesel::Connection;
use diesel::RunQueryDsl;
use time::Duration;
use uuid::Uuid;

#[test]
fn find_for_user_for_display() {
    let project = TestProject::new();
    let admin = project.create_user().finish();

    let connection = project.get_connection();
    let organization = project
        .create_organization()
        .with_fee_schedule(&project.create_fee_schedule().finish(admin.id))
        .finish();
    let event = project
        .create_event()
        .with_organization(&organization)
        .with_event_start(NaiveDate::from_ymd(2016, 7, 8).and_hms(9, 10, 11))
        .with_event_end(NaiveDate::from_ymd(2016, 7, 9).and_hms(9, 10, 11))
        .with_tickets()
        .with_ticket_pricing()
        .finish();
    let event2 = project
        .create_event()
        .with_organization(&organization)
        .with_event_start(NaiveDate::from_ymd(2017, 7, 8).and_hms(9, 10, 11))
        .with_event_end(NaiveDate::from_ymd(2017, 7, 9).and_hms(9, 10, 11))
        .with_tickets()
        .with_ticket_pricing()
        .finish();
    let user = project.create_user().finish();
    project
        .create_order()
        .for_user(&user)
        .quantity(2)
        .for_event(&event)
        .finish();
    let mut cart2 = project
        .create_order()
        .for_user(&user)
        .quantity(2)
        .for_event(&event2)
        .finish();

    // Order is not paid so tickets are not accessible
    assert!(TicketInstance::find_for_user_for_display(
        user.id,
        Some(event.id),
        None,
        None,
        connection
    )
    .unwrap()
    .is_empty());

    let total = cart2.calculate_total(connection).unwrap();
    cart2
        .add_external_payment(Some("test".to_string()), user.id, total, connection)
        .unwrap();

    let found_tickets =
        TicketInstance::find_for_user_for_display(user.id, Some(event.id), None, None, connection)
            .unwrap();
    assert_eq!(found_tickets.len(), 1);
    assert_eq!(found_tickets[0].0.id, event.id);
    assert_eq!(found_tickets[0].1.len(), 2);

    // other event
    let found_tickets =
        TicketInstance::find_for_user_for_display(user.id, Some(event2.id), None, None, connection)
            .unwrap();
    assert_eq!(found_tickets.len(), 1);
    assert_eq!(found_tickets[0].0.id, event2.id);
    assert_eq!(found_tickets[0].1.len(), 2);

    // no event specified
    let found_tickets =
        TicketInstance::find_for_user_for_display(user.id, None, None, None, connection).unwrap();
    assert_eq!(found_tickets.len(), 2);
    assert_eq!(found_tickets[0].0.id, event.id);
    assert_eq!(found_tickets[0].1.len(), 2);
    assert_eq!(found_tickets[1].0.id, event2.id);
    assert_eq!(found_tickets[1].1.len(), 2);

    // start date prior to both event starts
    let found_tickets = TicketInstance::find_for_user_for_display(
        user.id,
        None,
        Some(NaiveDate::from_ymd(2015, 7, 8).and_hms(9, 0, 11)),
        None,
        connection,
    )
    .unwrap();
    assert_eq!(found_tickets.len(), 2);
    assert_eq!(found_tickets[0].0.id, event.id);
    assert_eq!(found_tickets[0].1.len(), 2);
    assert_eq!(found_tickets[1].0.id, event2.id);
    assert_eq!(found_tickets[1].1.len(), 2);

    // start date filters out event

    let found_tickets = TicketInstance::find_for_user_for_display(
        user.id,
        None,
        Some(NaiveDate::from_ymd(2017, 7, 8).and_hms(9, 0, 11)),
        None,
        connection,
    )
    .unwrap();
    assert_eq!(found_tickets.len(), 1);
    assert_eq!(found_tickets[0].0.id, event2.id);
    assert_eq!(found_tickets[0].1.len(), 2);

    // end date filters out event
    let found_tickets = TicketInstance::find_for_user_for_display(
        user.id,
        None,
        None,
        Some(NaiveDate::from_ymd(2017, 7, 8).and_hms(9, 0, 11)),
        connection,
    )
    .unwrap();
    assert_eq!(found_tickets.len(), 1);
    assert_eq!(found_tickets[0].0.id, event.id);
    assert_eq!(found_tickets[0].1.len(), 2);
}

#[test]
fn release() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let creator = project.create_user().finish();
    let organization = project
        .create_organization()
        .with_fee_schedule(&project.create_fee_schedule().finish(creator.id))
        .finish();
    let event = project
        .create_event()
        .with_organization(&organization)
        .with_ticket_pricing()
        .finish();
    let user = project.create_user().finish();
    project
        .create_order()
        .for_event(&event)
        .for_user(&user)
        .quantity(1)
        .is_paid()
        .finish();
    let ticket = TicketInstance::find_for_user(user.id, connection)
        .unwrap()
        .remove(0);
    assert_eq!(ticket.status, TicketInstanceStatus::Purchased);
    TicketInstance::authorize_ticket_transfer(user.id, vec![ticket.id], 3600, connection).unwrap();
    assert!(ticket.release(connection).is_ok());

    // Reload ticket
    let ticket = TicketInstance::find(ticket.id, connection).unwrap();
    assert!(ticket.order_item_id.is_none());
    assert!(ticket.transfer_key.is_none());
    assert!(ticket.transfer_expiry_date.is_none());
    assert_eq!(ticket.status, TicketInstanceStatus::Available);
}

#[test]
fn set_wallet() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let creator = project.create_user().finish();
    let organization = project
        .create_organization()
        .with_fee_schedule(&project.create_fee_schedule().finish(creator.id))
        .finish();
    let event = project
        .create_event()
        .with_organization(&organization)
        .with_ticket_pricing()
        .finish();
    let user = project.create_user().finish();
    let user2 = project.create_user().finish();

    let user_wallet = Wallet::find_default_for_user(user.id, connection).unwrap();
    let user2_wallet = Wallet::find_default_for_user(user2.id, connection).unwrap();
    project
        .create_order()
        .for_event(&event)
        .for_user(&user)
        .quantity(1)
        .is_paid()
        .finish();
    let ticket = TicketInstance::find_for_user(user.id, connection)
        .unwrap()
        .remove(0);
    assert_eq!(ticket.wallet_id, user_wallet.id);
    ticket.set_wallet(&user2_wallet, connection).unwrap();
    let ticket = TicketInstance::find(ticket.id, connection).unwrap();
    assert_eq!(ticket.wallet_id, user2_wallet.id);
}

#[test]
fn was_transferred() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let creator = project.create_user().finish();
    let organization = project
        .create_organization()
        .with_fee_schedule(&project.create_fee_schedule().finish(creator.id))
        .finish();
    let event = project
        .create_event()
        .with_organization(&organization)
        .with_ticket_pricing()
        .finish();
    let user = project.create_user().finish();
    let user2 = project.create_user().finish();
    project
        .create_order()
        .for_event(&event)
        .for_user(&user)
        .quantity(1)
        .is_paid()
        .finish();
    let ticket = TicketInstance::find_for_user(user.id, connection)
        .unwrap()
        .remove(0);

    // Not transferred
    assert!(!ticket.was_transferred(connection).unwrap());

    let sender_wallet = Wallet::find_default_for_user(user.id, connection).unwrap();
    let receiver_wallet = Wallet::find_default_for_user(user2.id, connection).unwrap();
    let transfer_auth =
        TicketInstance::authorize_ticket_transfer(user.id, vec![ticket.id], 3600, connection)
            .unwrap();
    TicketInstance::receive_ticket_transfer(
        transfer_auth,
        &sender_wallet,
        &receiver_wallet.id,
        connection,
    )
    .unwrap();

    // Transferred
    assert!(ticket.was_transferred(connection).unwrap());
}

#[test]
fn find() {
    let project = TestProject::new();
    let org_admin = project.create_user().finish();

    let connection = project.get_connection();
    let organization = project
        .create_organization()
        .with_fee_schedule(&project.create_fee_schedule().finish(org_admin.id))
        .finish();
    let event = project
        .create_event()
        .with_organization(&organization)
        .with_tickets()
        .with_ticket_pricing()
        .finish();
    let user = project.create_user().finish();
    //let _d_user: DisplayUser = user.into();
    let mut cart = Order::find_or_create_cart(&user, connection).unwrap();
    let ticket_type = &event.ticket_types(connection).unwrap()[0];
    let ticket_pricing = ticket_type
        .current_ticket_pricing(false, connection)
        .unwrap();

    let display_event = event.for_display(connection).unwrap();
    cart.update_quantities(
        &[UpdateOrderItem {
            ticket_type_id: ticket_type.id,
            quantity: 1,
            redemption_code: None,
        }],
        false,
        false,
        connection,
    )
    .unwrap();
    let items = cart.items(&connection).unwrap();
    let order_item = items
        .iter()
        .find(|i| i.ticket_type_id == Some(ticket_type.id))
        .unwrap();
    let fee_schedule_range = ticket_type
        .fee_schedule(connection)
        .unwrap()
        .get_range(ticket_pricing.price_in_cents, connection)
        .unwrap();
    let ticket = TicketInstance::find_for_order_item(order_item.id, connection)
        .unwrap()
        .remove(0);
    let expected_ticket = DisplayTicket {
        id: ticket.id,
        order_id: cart.id,
        price_in_cents: (ticket_pricing.price_in_cents + fee_schedule_range.fee_in_cents) as u32,
        ticket_type_id: ticket_type.id,
        ticket_type_name: ticket_type.name.clone(),
        status: TicketInstanceStatus::Reserved,
        redeem_key: ticket.redeem_key,
        pending_transfer: false,
    };
    assert_eq!(
        (display_event, None, expected_ticket),
        TicketInstance::find_for_display(ticket.id, connection).unwrap()
    );
    assert!(TicketInstance::find(Uuid::new_v4(), connection).is_err());
}

#[test]
fn find_for_user() {
    let project = TestProject::new();
    let admin = project.create_user().finish();

    let connection = project.get_connection();
    let organization = project
        .create_organization()
        .with_fee_schedule(&project.create_fee_schedule().finish(admin.id))
        .finish();
    let event = project
        .create_event()
        .with_organization(&organization)
        .with_tickets()
        .with_ticket_pricing()
        .finish();

    let user = project.create_user().finish();
    let mut cart = Order::find_or_create_cart(&user, connection).unwrap();
    let ticket_type = &event.ticket_types(connection).unwrap()[0];
    cart.update_quantities(
        &[UpdateOrderItem {
            ticket_type_id: ticket_type.id,
            quantity: 5,
            redemption_code: None,
        }],
        false,
        false,
        connection,
    )
    .unwrap();

    let total = cart.calculate_total(connection).unwrap();
    cart.add_external_payment(Some("test".to_string()), user.id, total, connection)
        .unwrap();

    let tickets = TicketInstance::find_for_user(user.id, connection).unwrap();

    assert_eq!(tickets.len(), 5);
    assert!(TicketInstance::find(Uuid::new_v4(), connection).is_err());
}

#[test]
fn release_tickets() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let event = project.create_event().with_ticket_pricing().finish();
    let user = project.create_user().finish();
    let mut order = Order::find_or_create_cart(&user, connection).unwrap();
    let ticket_type_id = event.ticket_types(connection).unwrap()[0].id;
    order
        .update_quantities(
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

    let items = order.items(&connection).unwrap();
    let order_item = items
        .iter()
        .find(|i| i.ticket_type_id == Some(ticket_type_id))
        .unwrap();

    // Release tickets
    let released_tickets = TicketInstance::release_tickets(&order_item, 4, connection).unwrap();

    assert_eq!(released_tickets.len(), 4);
    assert!(released_tickets
        .iter()
        .filter(|&ticket| ticket.order_item_id == Some(order_item.id))
        .collect::<Vec<&TicketInstance>>()
        .is_empty());
    assert!(released_tickets
        .iter()
        .filter(|&ticket| ticket.reserved_until.is_some())
        .collect::<Vec<&TicketInstance>>()
        .is_empty());

    project
        .get_connection()
        .transaction::<Vec<TicketInstance>, Error, _>(|| {
            // Release requesting too many tickets
            let released_tickets = TicketInstance::release_tickets(&order_item, 7, connection);
            assert_eq!(released_tickets.unwrap_err().code, 7200,);

            Err(Error::RollbackTransaction)
        })
        .unwrap_err();
}

#[test]
fn redeem_ticket() {
    let project = TestProject::new();
    let admin = project.create_user().finish();

    let connection = project.get_connection();

    let organization = project
        .create_organization()
        .with_fee_schedule(&project.create_fee_schedule().finish(admin.id))
        .finish();
    let event = project
        .create_event()
        .with_organization(&organization)
        .with_ticket_pricing()
        .finish();
    let user = project.create_user().finish();
    project
        .create_order()
        .for_event(&event)
        .for_user(&user)
        .quantity(1)
        .is_paid()
        .finish();
    let ticket = TicketInstance::find_for_user(user.id, connection)
        .unwrap()
        .remove(0);

    let result1 =
        TicketInstance::redeem_ticket(ticket.id, "WrongKey".to_string(), connection).unwrap();
    assert_eq!(result1, RedeemResults::TicketInvalid);
    let result2 =
        TicketInstance::redeem_ticket(ticket.id, ticket.redeem_key.unwrap(), connection).unwrap();
    assert_eq!(result2, RedeemResults::TicketRedeemSuccess);
}

#[test]
fn show_redeemable_ticket() {
    let project = TestProject::new();
    let admin = project.create_user().finish();

    let connection = project.get_connection();

    let organization = project
        .create_organization()
        .with_fee_schedule(&project.create_fee_schedule().finish(admin.id))
        .finish();
    let venue = project.create_venue().finish();
    let event = project
        .create_event()
        .with_organization(&organization)
        .with_ticket_pricing()
        .with_venue(&venue)
        .finish();
    let user = project.create_user().finish();
    project
        .create_order()
        .for_event(&event)
        .for_user(&user)
        .quantity(1)
        .is_paid()
        .finish();
    let ticket = TicketInstance::find_for_user(user.id, connection)
        .unwrap()
        .remove(0);

    //make redeem date in the future
    let new_event_redeem_date = EventEditableAttributes {
        redeem_date: Some(NaiveDateTime::from(
            Utc::now().naive_utc() + Duration::days(2),
        )),
        ..Default::default()
    };

    let event = event.update(new_event_redeem_date, connection).unwrap();

    let result = TicketInstance::show_redeemable_ticket(ticket.id, connection).unwrap();
    assert!(result.redeem_key.is_none());

    //make redeem date in the past
    let new_event_redeem_date = EventEditableAttributes {
        redeem_date: Some(NaiveDateTime::from(
            Utc::now().naive_utc() - Duration::days(2),
        )),
        ..Default::default()
    };

    let event = event.update(new_event_redeem_date, connection).unwrap();

    let result = TicketInstance::show_redeemable_ticket(ticket.id, connection).unwrap();
    assert!(result.redeem_key.is_some());

    // Set order on behalf of (should show user information for the on_behalf_of_user user)
    let user2 = project.create_user().finish();
    let order = project
        .create_order()
        .for_event(&event)
        .for_user(&user)
        .on_behalf_of_user(&user2)
        .quantity(1)
        .is_paid()
        .finish();
    let ticket_type = &event.ticket_types(&connection).unwrap()[0];
    let ticket = order.tickets(ticket_type.id, connection).unwrap().remove(0);
    let result = TicketInstance::show_redeemable_ticket(ticket.id, connection).unwrap();
    assert_eq!(result.user_id, Some(user2.id));
}

#[test]
fn authorize_ticket_transfer() {
    let project = TestProject::new();
    let admin = project.create_user().finish();

    let connection = project.get_connection();
    let organization = project
        .create_organization()
        .with_fee_schedule(&project.create_fee_schedule().finish(admin.id))
        .finish();
    let event = project
        .create_event()
        .with_organization(&organization)
        .with_tickets()
        .with_ticket_pricing()
        .finish();

    let user = project.create_user().finish();
    let mut cart = Order::find_or_create_cart(&user, connection).unwrap();
    let ticket_type = &event.ticket_types(connection).unwrap()[0];
    cart.update_quantities(
        &[UpdateOrderItem {
            ticket_type_id: ticket_type.id,
            quantity: 5,
            redemption_code: None,
        }],
        false,
        false,
        connection,
    )
    .unwrap();
    let total = cart.calculate_total(connection).unwrap();

    cart.add_external_payment(Some("test".to_string()), user.id, total, connection)
        .unwrap();

    let tickets = TicketInstance::find_for_user(user.id, connection).unwrap();

    assert_eq!(tickets.len(), 5);
    //try with a ticket that does not exist in the list

    let tickets = TicketInstance::find_for_user(user.id, connection).unwrap();

    let mut ticket_ids: Vec<Uuid> = tickets.iter().map(|t| t.id).collect();
    ticket_ids.push(Uuid::new_v4());

    let transfer_auth2 =
        TicketInstance::authorize_ticket_transfer(user.id, ticket_ids, 24, connection);

    assert!(transfer_auth2.is_err());

    //Now try with tickets that the user does own

    let ticket_ids: Vec<Uuid> = tickets.iter().map(|t| t.id).collect();

    let transfer_auth3 =
        TicketInstance::authorize_ticket_transfer(user.id, ticket_ids, 24, connection).unwrap();

    assert_eq!(transfer_auth3.sender_user_id, user.id);
}

#[test]
fn receive_ticket_transfer() {
    let project = TestProject::new();
    let admin = project.create_user().finish();

    let connection = project.get_connection();
    let organization = project
        .create_organization()
        .with_fee_schedule(&project.create_fee_schedule().finish(admin.id))
        .finish();
    let event = project
        .create_event()
        .with_organization(&organization)
        .with_tickets()
        .with_ticket_pricing()
        .finish();

    let user = project.create_user().finish();
    let mut cart = Order::find_or_create_cart(&user, connection).unwrap();
    let ticket_type = &event.ticket_types(connection).unwrap()[0];
    cart.update_quantities(
        &[UpdateOrderItem {
            ticket_type_id: ticket_type.id,
            quantity: 5,
            redemption_code: None,
        }],
        false,
        false,
        connection,
    )
    .unwrap();
    let total = cart.calculate_total(connection).unwrap();

    cart.add_external_payment(Some("test".to_string()), user.id, total, connection)
        .unwrap();
    let tickets = TicketInstance::find_for_user(user.id, connection).unwrap();
    let ticket_ids: Vec<Uuid> = tickets.iter().map(|t| t.id).collect();

    let user2 = project.create_user().finish();
    //try receive ones that are expired
    let transfer_auth =
        TicketInstance::authorize_ticket_transfer(user.id, ticket_ids.clone(), 0, connection)
            .unwrap();

    let _q: Vec<TicketInstance> = diesel::sql_query(
        r#"
        UPDATE ticket_instances
        SET transfer_expiry_date = '2018-06-06 09:49:09.643207'
        WHERE id = $1;
        "#,
    )
    .bind::<sql_types::Uuid, _>(ticket_ids[0])
    .get_results(connection)
    .unwrap();

    let sender_wallet =
        Wallet::find_default_for_user(transfer_auth.sender_user_id, connection).unwrap();
    let receiver_wallet = Wallet::find_default_for_user(user2.id, connection).unwrap();

    let receive_auth2 = TicketInstance::receive_ticket_transfer(
        transfer_auth,
        &sender_wallet,
        &receiver_wallet.id,
        connection,
    );

    assert!(receive_auth2.is_err());

    //try receive the wrong number of tickets (too few)
    let transfer_auth =
        TicketInstance::authorize_ticket_transfer(user.id, ticket_ids.clone(), 3600, connection)
            .unwrap();

    let mut wrong_auth = transfer_auth.clone();
    wrong_auth.num_tickets = 4;
    let receive_auth1 = TicketInstance::receive_ticket_transfer(
        wrong_auth,
        &sender_wallet,
        &receiver_wallet.id,
        connection,
    );
    assert!(receive_auth1.is_err());

    //legit receive tickets
    let _receive_auth3 = TicketInstance::receive_ticket_transfer(
        transfer_auth,
        &sender_wallet,
        &receiver_wallet.id,
        connection,
    );

    //Look if one of the tickets does have the new wallet_id
    let receive_wallet = Wallet::find_default_for_user(user2.id, connection).unwrap();

    let received_ticket = TicketInstance::find(ticket_ids[0], connection).unwrap();

    assert_eq!(receive_wallet.id, received_ticket.wallet_id);
}
