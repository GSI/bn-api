use chrono::prelude::*;
use diesel;
use diesel::dsl::{exists, select};
use diesel::expression::dsl;
use diesel::expression::sql_literal::sql;
use diesel::pg::types::sql_types::Array;
use diesel::prelude::*;
use diesel::sql_types;
use diesel::sql_types::{BigInt, Bool, Integer, Nullable, Text, Uuid as dUuid};
use itertools::Itertools;
use log::Level;
use models::*;
use schema::{events, order_items, orders, organizations, payments, users};
use serde_json;
use std::borrow::Cow;
use std::cmp;
use std::collections::HashMap;
use time::Duration;
use utils::errors::*;
use uuid::Uuid;
use validator::ValidationErrors;
use validators::*;

const CART_EXPIRY_TIME_MINUTES: i64 = 15;
const ORDER_NUMBER_LENGTH: usize = 8;

#[derive(Associations, Debug, Identifiable, PartialEq, Queryable)]
#[belongs_to(User)]
pub struct Order {
    pub id: Uuid,
    pub user_id: Uuid,
    pub status: OrderStatus,
    order_type: String,
    pub order_date: NaiveDateTime,
    pub expires_at: Option<NaiveDateTime>,
    pub version: i64,
    pub note: Option<String>,
    pub on_behalf_of_user_id: Option<Uuid>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub paid_at: Option<NaiveDateTime>,
    pub box_office_pricing: bool,
}

#[derive(Insertable)]
#[table_name = "orders"]
pub struct NewOrder {
    user_id: Uuid,
    status: OrderStatus,
    expires_at: Option<NaiveDateTime>,
    order_type: String,
}

impl NewOrder {
    pub fn commit(&self, conn: &PgConnection) -> Result<Order, DatabaseError> {
        use schema::orders;
        DatabaseError::wrap(
            ErrorCode::InsertError,
            "Could not create new order",
            diesel::insert_into(orders::table)
                .values(self)
                .get_result(conn),
        )
    }
}

#[derive(AsChangeset, Deserialize)]
#[table_name = "orders"]
pub struct UpdateOrderAttributes {
    #[serde(default, deserialize_with = "double_option_deserialize_unless_blank")]
    pub note: Option<Option<String>>,
}

#[derive(Deserialize, Serialize)]
pub struct RefundItem {
    pub order_item_id: Uuid,
    pub ticket_instance_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, PartialEq, Queryable, QueryableByName, Serialize)]
pub struct OrderDetailsLineItem {
    #[sql_type = "Nullable<dUuid>"]
    pub ticket_instance_id: Option<Uuid>,
    #[sql_type = "dUuid"]
    pub order_item_id: Uuid,
    #[sql_type = "Text"]
    pub description: String,
    #[sql_type = "BigInt"]
    pub ticket_price_in_cents: i64,
    #[sql_type = "BigInt"]
    pub fees_price_in_cents: i64,
    #[sql_type = "BigInt"]
    pub total_price_in_cents: i64,
    #[sql_type = "Text"]
    pub status: String,
    #[sql_type = "Bool"]
    pub refundable: bool,
}

impl Order {
    pub fn destroy(&self, conn: &PgConnection) -> Result<usize, DatabaseError> {
        let cart_user: Option<User> = users::table
            .filter(users::last_cart_id.eq(self.id))
            .get_result(conn)
            .to_db_error(
                ErrorCode::QueryError,
                "Could not find user attached to this cart",
            )
            .optional()?;

        if let Some(user) = cart_user {
            user.update_last_cart(None, conn)?;
        }

        DatabaseError::wrap(
            ErrorCode::DeleteError,
            "Failed to delete order record",
            diesel::delete(self).execute(conn),
        )
    }

    pub(crate) fn destroy_item(
        &self,
        item_id: Uuid,
        conn: &PgConnection,
    ) -> Result<(), DatabaseError> {
        if self.status != OrderStatus::Draft {
            return DatabaseError::business_process_error(
                "Cannot delete an order item for an order that is not in draft",
            );
        }

        // delete children order items
        diesel::delete(order_items::table.filter(order_items::parent_id.eq(item_id)))
            .execute(conn)
            .map(|_| ())
            .to_db_error(ErrorCode::DeleteError, "Could not delete child order item")?;

        diesel::delete(order_items::table.filter(order_items::id.eq(item_id)))
            .execute(conn)
            .map(|_| ())
            .to_db_error(ErrorCode::DeleteError, "Could not delete order item")
    }

    pub fn refund(
        &self,
        refund_items: Vec<RefundItem>,
        conn: &PgConnection,
    ) -> Result<u32, DatabaseError> {
        let mut total_to_be_refunded: u32 = 0;
        for refund_item in refund_items {
            let mut order_item = OrderItem::find(refund_item.order_item_id, conn)?;

            if order_item.order_id != self.id {
                return DatabaseError::business_process_error(
                    "Order item id does not belong to this order",
                );
            }

            let ticket_instance = match refund_item.ticket_instance_id {
                Some(id) => Some(TicketInstance::find(id, conn)?),
                None => None,
            };

            if order_item.item_type == OrderItemTypes::Tickets
                || order_item.item_type == OrderItemTypes::PerUnitFees
            {
                match ticket_instance {
                    None => {
                        return DatabaseError::business_process_error(
                            "Ticket id required when refunding ticket related order item",
                        )
                    }
                    Some(ref ticket_instance) => {
                        let mut refunded_ticket =
                            RefundedTicket::find_or_create_by_ticket_instance(
                                ticket_instance,
                                conn,
                            )?;

                        if refunded_ticket.ticket_refunded_at.is_some()
                            || (refunded_ticket.fee_refunded_at.is_some()
                                && order_item.item_type == OrderItemTypes::PerUnitFees)
                        {
                            return DatabaseError::business_process_error("Already refunded");
                        } else if ticket_instance.was_transferred(conn)? {
                            return DatabaseError::business_process_error(
                                "Ticket was transferred so ineligible for refund",
                            );
                        }

                        let only_refund_fees = order_item.item_type == OrderItemTypes::PerUnitFees;
                        let refund_fees = refunded_ticket.fee_refunded_at.is_none();
                        refunded_ticket.mark_refunded(only_refund_fees, conn)?;

                        if ticket_instance.status != TicketInstanceStatus::Redeemed {
                            ticket_instance.release(conn)?;
                        }

                        total_to_be_refunded += order_item.refund_one_unit(refund_fees, conn)?;
                    }
                }
            } else {
                total_to_be_refunded += order_item.refund_one_unit(true, conn)?;
            }
        }

        for mut event_fee_item in self.event_fee_items_with_no_associated_items(conn)? {
            total_to_be_refunded += event_fee_item.refund_one_unit(true, conn)?;
        }

        Ok(total_to_be_refunded)
    }

    fn event_fee_items_with_no_associated_items(
        &self,
        conn: &PgConnection,
    ) -> Result<Vec<OrderItem>, DatabaseError> {
        order_items::table
            .filter(order_items::refunded_quantity.ne(order_items::quantity))
            .filter(order_items::order_id.eq(self.id))
            .filter(order_items::item_type.eq(OrderItemTypes::EventFees))
            .filter(sql("not exists(
                select id from order_items oi2
                where oi2.order_id = order_items.order_id
                and oi2.event_id = order_items.event_id
                and item_type <> 'EventFees'
                and oi2.refunded_quantity <> oi2.quantity
            )"))
            .select(order_items::all_columns)
            .load(conn)
            .to_db_error(
                ErrorCode::QueryError,
                "Could not check if order only contains event fees",
            )
    }

    pub fn details(
        &self,
        organization_ids: Vec<Uuid>,
        conn: &PgConnection,
    ) -> Result<Vec<OrderDetailsLineItem>, DatabaseError> {
        let query = include_str!("../queries/order_details.sql");
        diesel::sql_query(query)
            .bind::<dUuid, _>(self.id)
            .bind::<Array<dUuid>, _>(organization_ids)
            .load(conn)
            .to_db_error(ErrorCode::QueryError, "Could not load order items")
    }

    pub fn organizations(&self, conn: &PgConnection) -> Result<Vec<Organization>, DatabaseError> {
        organizations::table
            .inner_join(events::table.on(events::organization_id.eq(organizations::id)))
            .inner_join(order_items::table.on(order_items::event_id.eq(events::id.nullable())))
            .filter(order_items::order_id.eq(self.id))
            .select(organizations::all_columns)
            .order_by(organizations::name.asc())
            .distinct()
            .load(conn)
            .to_db_error(ErrorCode::QueryError, "Error loading organizations")
    }

    pub fn payments(&self, conn: &PgConnection) -> Result<Vec<Payment>, DatabaseError> {
        payments::table
            .filter(payments::order_id.eq(self.id))
            .load(conn)
            .to_db_error(ErrorCode::QueryError, "Error loading payments")
    }

    pub fn find_or_create_cart(user: &User, conn: &PgConnection) -> Result<Order, DatabaseError> {
        // Do a quick check to find the cart linked to the user.
        let cart = Order::find_cart_for_user(user.id, conn)?;

        if let Some(cart) = cart {
            return Ok(cart);
        }

        // Cart either does not exist, expired or was paid up.
        // A number of threads might reach here at the same time, so we
        // need to do a bit of concurrency checking.

        let query = r#"
            INSERT INTO Orders (user_id, status, expires_at, order_type)
            SELECT $1 as user_id, 'Draft' as status, null as expires_at, 'Cart' as order_type
            WHERE NOT EXISTS
            ( SELECT o.id FROM orders o
                WHERE o.user_id = $1
                AND o.status = 'Draft'
                AND o.order_type = 'Cart'
                AND o.expires_at > now())
            RETURNING id;
        "#;

        #[derive(QueryableByName)]
        struct R {
            #[sql_type = "Nullable<dUuid>"]
            id: Option<Uuid>,
        }

        let cart_id: Vec<R> = diesel::sql_query(query)
            .bind::<sql_types::Uuid, _>(user.id)
            .get_results(conn)
            .to_db_error(ErrorCode::QueryError, "Could not find or create cart")?;

        if cart_id.is_empty() || cart_id[0].id.is_none() || cart_id.len() > 1 {
            // Another thread has created a cart
            return DatabaseError::concurrency_error(&format!(
                "Possible race condition when creating a cart for a user. Number of carts returned: {}",
                cart_id.len()
            ));
        }

        let cart_id = cart_id[0].id;

        // This will also row lock the user row to detect that another thread has not
        // created another cart in the mean time
        user.update_last_cart(cart_id, conn)?;

        // Finally return the actual order
        Order::find(cart_id.unwrap(), conn)
    }

    pub fn find_cart_for_user(
        user_id: Uuid,
        conn: &PgConnection,
    ) -> Result<Option<Order>, DatabaseError> {
        users::table
            .inner_join(orders::table.on(users::last_cart_id.eq(orders::id.nullable())))
            .filter(users::id.eq(user_id))
            .filter(orders::user_id.eq(user_id))
            .filter(orders::status.eq("Draft"))
            .filter(orders::order_type.eq("Cart"))
            .filter(
                orders::expires_at
                    .is_null()
                    .or(orders::expires_at.ge(dsl::now.nullable())),
            )
            .select(orders::all_columns)
            .first(conn)
            .to_db_error(ErrorCode::QueryError, "Could not load cart for user")
            .optional()
    }

    pub fn find(id: Uuid, conn: &PgConnection) -> Result<Order, DatabaseError> {
        orders::table
            .filter(orders::id.eq(id))
            .first(conn)
            .to_db_error(ErrorCode::QueryError, "Could not find order")
    }

    pub fn set_expiry(&mut self, conn: &PgConnection) -> Result<(), DatabaseError> {
        self.expires_at =
            Some(Utc::now().naive_utc() + Duration::minutes(CART_EXPIRY_TIME_MINUTES));
        self.updated_at = Utc::now().naive_utc();

        let affected_rows = diesel::update(
            orders::table.filter(
                orders::id
                    .eq(self.id)
                    .and(orders::version.eq(self.version))
                    .and(orders::expires_at.is_null()),
            ),
        )
        .set((
            orders::expires_at.eq(self.expires_at),
            orders::updated_at.eq(self.updated_at),
        ))
        .execute(conn)
        .to_db_error(ErrorCode::UpdateError, "Could not update expiry time")?;
        if affected_rows != 1 {
            return DatabaseError::concurrency_error("Could not update expiry time.");
        }
        Ok(())
    }

    pub fn remove_expiry(&mut self, conn: &PgConnection) -> Result<(), DatabaseError> {
        self.updated_at = Utc::now().naive_utc();
        self.expires_at = None;
        let affected_rows = diesel::update(
            orders::table.filter(
                orders::id
                    .eq(self.id)
                    .and(orders::version.eq(self.version))
                    .and(
                        orders::expires_at
                            .is_null()
                            .or(orders::expires_at.gt(Some(Utc::now().naive_utc()))),
                    ),
            ),
        )
        .set((
            orders::expires_at.eq::<Option<NaiveDateTime>>(None),
            orders::updated_at.eq(self.updated_at),
        ))
        .execute(conn)
        .to_db_error(ErrorCode::UpdateError, "Could not update expiry time")?;
        if affected_rows != 1 {
            return DatabaseError::concurrency_error("Could not update expiry time.");
        }

        Ok(())
    }

    pub fn order_number(&self) -> String {
        Order::parse_order_number(self.id)
    }

    pub fn parse_order_number(id: Uuid) -> String {
        let id_string = id.to_string();
        id_string[id_string.len() - ORDER_NUMBER_LENGTH..].to_string()
    }

    pub fn update(
        self,
        attrs: UpdateOrderAttributes,
        conn: &PgConnection,
    ) -> Result<Order, DatabaseError> {
        diesel::update(&self)
            .set((attrs, orders::updated_at.eq(dsl::now)))
            .execute(conn)
            .to_db_error(ErrorCode::UpdateError, "Could not update order")?;
        Order::find(self.id, conn)
    }

    pub fn clear_cart(&self, conn: &PgConnection) -> Result<(), DatabaseError> {
        jlog!(Level::Debug, "Clearing cart");
        for mut current_line in self.items(conn)? {
            if current_line.item_type != OrderItemTypes::Tickets {
                continue;
            }
            TicketInstance::release_tickets(&current_line, current_line.quantity as u32, conn)?;
            self.destroy_item(current_line.id, conn)?;
        }
        Ok(())
    }

    pub fn update_quantities(
        &mut self,
        items: &[UpdateOrderItem],
        box_office_pricing: bool,
        remove_others: bool,
        conn: &PgConnection,
    ) -> Result<(), DatabaseError> {
        self.lock_version(conn)?;

        if box_office_pricing != self.box_office_pricing {
            self.clear_cart(conn)?;
            self.update_box_office_pricing(box_office_pricing, conn)?;
        }

        let current_items = self.items(conn)?;

        #[derive(Debug)]
        struct LimitCheck {
            ticket_type_id: Uuid,
            event_id: Uuid,
            limit_per_person: i32,
        }
        let mut check_ticket_limits: Vec<LimitCheck> = vec![];

        let mut mapped = vec![];
        for (index, item) in items.iter().enumerate() {
            mapped.push(match &item.redemption_code {
                Some(r) => match Hold::find_by_redemption_code(r, conn).optional()? {
                    Some(hold) => (Some(index), Some(hold.id), Some(hold), item),
                    None => {
                        return DatabaseError::validation_error(
                            "redemption_code",
                            "Redemption code is not valid",
                        )
                    }
                },
                None => (Some(index), None, None, item),
            });
        }

        for mut current_line in current_items {
            if current_line.item_type != OrderItemTypes::Tickets {
                continue;
            }

            let mut index_to_remove: Option<usize> = None;
            {
                let matching_result: Vec<&(
                    Option<usize>,
                    Option<Uuid>,
                    Option<Hold>,
                    &UpdateOrderItem,
                )> = mapped
                    .iter()
                    .filter(|(index, hold_id, _, item)| {
                        index.is_some()
                            && Some(item.ticket_type_id) == current_line.ticket_type_id
                            && *hold_id == current_line.hold_id
                    })
                    .collect();
                let matching_result = matching_result.first();

                if let Some(matching_result) = matching_result {
                    jlog!(Level::Debug, "Found an existing cart item, replacing");
                    let (index, hold_id, hold, mut matching_line) = matching_result;
                    index_to_remove = *index;
                    if current_line.quantity as u32 > matching_line.quantity {
                        jlog!(Level::Debug, "Reducing quantity of cart item");
                        TicketInstance::release_tickets(
                            &current_line,
                            current_line.quantity as u32 - matching_line.quantity,
                            conn,
                        )?;
                        current_line.quantity = matching_line.quantity as i64;
                        current_line.update(conn)?;
                        if current_line.quantity == 0 {
                            jlog!(Level::Debug, "Cart item has 0 quantity, deleting it");
                            self.destroy_item(current_line.id, conn)?;
                        }
                    } else if (current_line.quantity as u32) < matching_line.quantity {
                        jlog!(Level::Debug, "Increasing quantity of cart item");
                        // Ticket pricing might have changed since we added the previous item.
                        // In future we may want to use the ticket pricing at the time the order was created.

                        // TODO: Fetch the ticket type and pricing in one go.
                        let ticket_type_id = current_line.ticket_type_id.unwrap();
                        let ticket_pricing = TicketPricing::get_current_ticket_pricing(
                            ticket_type_id,
                            box_office_pricing,
                            false,
                            conn,
                        )?;
                        let ticket_type = TicketType::find(ticket_type_id, conn)?;

                        check_ticket_limits.push(LimitCheck {
                            limit_per_person: ticket_type.limit_per_person.clone(),
                            ticket_type_id: ticket_type.id.clone(),
                            event_id: ticket_type.event_id.clone(),
                        });

                        // TODO: Move this to an external processer
                        if Some(ticket_pricing.id) != current_line.ticket_pricing_id {
                            let mut price_in_cents = ticket_pricing.price_in_cents;
                            if let Some(h) = hold.as_ref() {
                                let discount = h.discount_in_cents;
                                let hold_type = h.hold_type;
                                price_in_cents = match hold_type {
                                    HoldTypes::Discount => {
                                        cmp::max(0, price_in_cents - discount.unwrap_or(0))
                                    }
                                    HoldTypes::Comp => 0,
                                }
                            }

                            let order_item = NewTicketsOrderItem {
                                order_id: self.id,
                                item_type: OrderItemTypes::Tickets,
                                quantity: matching_line.quantity as i64,
                                ticket_type_id: ticket_type.id,
                                ticket_pricing_id: ticket_pricing.id,
                                event_id: Some(ticket_type.event_id),
                                unit_price_in_cents: price_in_cents,
                                hold_id: *hold_id,
                                code_id: None,
                            }
                            .commit(conn)?;
                            TicketInstance::reserve_tickets(
                                &order_item,
                                self.expires_at,
                                ticket_type_id,
                                *hold_id,
                                matching_line.quantity - current_line.quantity as u32,
                                conn,
                            )?;
                        } else {
                            TicketInstance::reserve_tickets(
                                &current_line,
                                self.expires_at,
                                ticket_type_id,
                                *hold_id,
                                matching_line.quantity - current_line.quantity as u32,
                                conn,
                            )?;
                            current_line.quantity = matching_line.quantity as i64;
                            current_line.update(conn)?;
                        }
                    }
                } else if remove_others {
                    jlog!(Level::Debug, "Removing extra tickets because remove others was called.", { "order_item.id": current_line.id, "ticket_type_id": current_line.ticket_type_id});
                    jlog!(Level::Debug, "Reducing quantity of cart item");
                    TicketInstance::release_tickets(
                        &current_line,
                        current_line.quantity as u32,
                        conn,
                    )?;
                    self.destroy_item(current_line.id, conn)?;
                }
            }
            if let Some(index) = index_to_remove {
                mapped[index].0 = None;
            }
        }

        // Set cart expiration time if not currently set (empty carts have no expiration)
        if self.expires_at.is_none() {
            self.set_expiry(conn)?;
        }

        for (index, hold_id, hold, new_line) in mapped {
            if new_line.quantity == 0 || index.is_none() {
                continue;
            }

            jlog!(Level::Debug, "Adding new cart items");
            let ticket_pricing = TicketPricing::get_current_ticket_pricing(
                new_line.ticket_type_id,
                box_office_pricing,
                false,
                conn,
            )?;
            let ticket_type = TicketType::find(new_line.ticket_type_id, conn)?;

            check_ticket_limits.push(LimitCheck {
                limit_per_person: ticket_type.limit_per_person.clone(),
                ticket_type_id: ticket_type.id.clone(),
                event_id: ticket_type.event_id.clone(),
            });

            let mut price_in_cents = ticket_pricing.price_in_cents;
            if let Some(h) = hold.as_ref() {
                let discount = h.discount_in_cents;
                let hold_type = h.hold_type;
                price_in_cents = match hold_type {
                    HoldTypes::Discount => cmp::max(0, price_in_cents - discount.unwrap_or(0)),
                    HoldTypes::Comp => 0,
                }
            }
            // TODO: Move this to an external processer
            let order_item = NewTicketsOrderItem {
                order_id: self.id,
                item_type: OrderItemTypes::Tickets,
                quantity: new_line.quantity as i64,
                ticket_type_id: ticket_type.id,
                ticket_pricing_id: ticket_pricing.id,
                event_id: Some(ticket_type.event_id),
                unit_price_in_cents: price_in_cents,
                hold_id: hold_id,
                code_id: None,
            }
            .commit(conn)?;

            TicketInstance::reserve_tickets(
                &order_item,
                self.expires_at,
                new_line.ticket_type_id,
                hold_id,
                new_line.quantity,
                conn,
            )?;
        }

        // if the cart is empty at this point, it is effectively a new cart, remove expiration
        if self.items(conn)?.len() == 0 {
            self.remove_expiry(conn)?;
        }

        for limit_check in check_ticket_limits {
            let quantities_ordered =
                Order::quantity_for_user_for_event(&self.user_id, &limit_check.event_id, &conn)?;
            if &limit_check.limit_per_person > &0
                && quantities_ordered.contains_key(&limit_check.ticket_type_id)
            {
                match quantities_ordered.get(&limit_check.ticket_type_id) {
                    Some(ordered_quantity) => {
                        if ordered_quantity > &limit_check.limit_per_person {
                            let mut error = create_validation_error(
                                "limit_per_person_exceeded",
                                "Exceeded limit per person per event",
                            );
                            error.add_param(
                                Cow::from("limit_per_person"),
                                &limit_check.limit_per_person,
                            );
                            error.add_param(
                                Cow::from("ticket_type_id"),
                                &limit_check.ticket_type_id,
                            );
                            error.add_param(Cow::from("attempted_quantity"), ordered_quantity);
                            let mut errors = ValidationErrors::new();
                            errors.add("quantity", error);
                            return Err(errors.into());
                        }
                    }
                    None => {}
                };
            }
        }

        self.update_fees(conn)?;

        Ok(())
    }

    pub fn has_items(&self, conn: &PgConnection) -> Result<bool, DatabaseError> {
        select(exists(
            order_items::table.filter(order_items::order_id.eq(self.id)),
        ))
        .get_result(conn)
        .to_db_error(
            ErrorCode::QueryError,
            "Could not check if order items exist",
        )
    }

    pub fn update_fees(&self, conn: &PgConnection) -> Result<(), DatabaseError> {
        let items = self.items(conn)?;

        for o in items {
            match o.item_type {
                OrderItemTypes::EventFees => self.destroy_item(o.id, conn)?,
                _ => {}
            }
        }
        for ((event_id, hold_id), items) in self
            .items(conn)?
            .iter()
            .filter(|i| i.event_id.is_some())
            .group_by(|i| (i.event_id, i.hold_id))
            .into_iter()
        {
            if event_id.is_none() {
                continue;
            }
            if hold_id.is_some() {
                let hold = Hold::find(hold_id.unwrap(), conn)?;
                if hold.hold_type == HoldTypes::Comp {
                    continue;
                }
            }

            let event = Event::find(event_id.unwrap(), conn)?;

            let mut all_zero_price = true;

            for o in items {
                match o.item_type {
                    OrderItemTypes::Tickets => {
                        o.update_fees(&self, conn)?;
                        if o.unit_price_in_cents > 0 {
                            all_zero_price = false;
                        }
                    }
                    _ => {}
                }
            }

            if !all_zero_price {
                let mut new_event_fee = NewFeesOrderItem {
                    order_id: self.id,
                    item_type: OrderItemTypes::EventFees,
                    event_id: Some(event.id),
                    unit_price_in_cents: 0,
                    fee_schedule_range_id: None,
                    company_fee_in_cents: 0,
                    client_fee_in_cents: 0,
                    quantity: 1,
                    parent_id: None,
                };
                if event.fee_in_cents > 0 {
                    //we dont want to create 0 fee order item
                    new_event_fee.company_fee_in_cents = event.company_fee_in_cents;
                    new_event_fee.client_fee_in_cents = event.client_fee_in_cents;
                    new_event_fee.unit_price_in_cents =
                        event.client_fee_in_cents + event.company_fee_in_cents;
                    new_event_fee.commit(conn)?;
                }
            }
        }

        Ok(())
    }

    pub fn quantity_for_user_for_event(
        user_id: &Uuid,
        event_id: &Uuid,
        conn: &PgConnection,
    ) -> Result<HashMap<Uuid, i32>, DatabaseError> {
        let mut ticket_type_totals: HashMap<Uuid, i32> = HashMap::new();

        let query = include_str!("../queries/quantity_of_tickets_per_user_per_event.sql");
        let order_items_for_user: Vec<ResultForTicketTypeTotal> = diesel::sql_query(query)
            .bind::<diesel::sql_types::Uuid, _>(user_id)
            .bind::<diesel::sql_types::Uuid, _>(event_id)
            .load::<ResultForTicketTypeTotal>(conn)
            .to_db_error(ErrorCode::QueryError, "Could not load orders")?;

        for result_for_ticket in &order_items_for_user {
            ticket_type_totals.insert(
                result_for_ticket.ticket_type_id.unwrap(),
                result_for_ticket.total_quantity,
            );
        }

        Ok(ticket_type_totals)
    }

    pub fn find_for_user_for_display(
        user_id: Uuid,
        conn: &PgConnection,
    ) -> Result<Vec<DisplayOrder>, DatabaseError> {
        use schema::*;
        let orders: Vec<Order> = orders::table
            .filter(
                orders::user_id
                    .eq(user_id)
                    .or(orders::on_behalf_of_user_id.eq(user_id)),
            )
            .filter(orders::status.ne(OrderStatus::Draft))
            .order_by(orders::order_date.desc())
            .load(conn)
            .to_db_error(ErrorCode::QueryError, "Could not load orders")?;
        let mut r = Vec::<DisplayOrder>::new();
        for order in orders {
            r.push(order.for_display(conn)?);
        }
        Ok(r)
    }

    pub fn items(&self, conn: &PgConnection) -> Result<Vec<OrderItem>, DatabaseError> {
        OrderItem::find_for_order(self.id, conn)
    }

    pub fn tickets(
        &self,
        ticket_type_id: Uuid,
        conn: &PgConnection,
    ) -> Result<Vec<TicketInstance>, DatabaseError> {
        let items = self.items(conn)?;
        let mut tickets: Vec<OrderItem> = Vec::new();
        for ci in items {
            if ci.item_type == OrderItemTypes::Tickets && ci.ticket_type_id == Some(ticket_type_id)
            {
                tickets.push(ci);
            }
        }

        let mut result: Vec<TicketInstance> = vec![];
        for t in tickets {
            let mut instances = TicketInstance::find_for_order_item(t.id, conn)?;
            result.append(&mut instances);
        }

        Ok(result)
    }

    pub fn for_display(&self, conn: &PgConnection) -> Result<DisplayOrder, DatabaseError> {
        let now = Utc::now().naive_utc();
        let seconds_until_expiry = self.expires_at.map(|expires_at| {
            if expires_at >= now {
                let duration = expires_at.signed_duration_since(now);
                duration.num_seconds() as u32
            } else {
                0
            }
        });

        let items = self.items(conn)?; //Would be nice to use the items_for_display call below but it doesnt include the event_id
        let mut unique_events: Vec<Uuid> = items.iter().filter_map(|i| i.event_id).collect();

        unique_events.sort();
        unique_events.dedup(); //Compiler doesnt like chaining these...

        let mut limited_tickets_remaining: Vec<TicketsRemaining> = Vec::new();

        for e in unique_events {
            let tickets_bought = Order::quantity_for_user_for_event(&self.user_id, &e, conn)?;
            for (tt_id, num) in tickets_bought {
                let limit = TicketType::find(tt_id, conn)?.limit_per_person;
                if limit > 0 {
                    limited_tickets_remaining.push(TicketsRemaining {
                        ticket_type_id: tt_id,
                        tickets_remaining: limit - num,
                    });
                }
            }
        }

        Ok(DisplayOrder {
            id: self.id,
            status: self.status.clone(),
            date: self.order_date,
            expires_at: self.expires_at,
            items: self.items_for_display(conn)?,
            limited_tickets_remaining,
            total_in_cents: self.calculate_total(conn)?,
            seconds_until_expiry,
            user_id: self.user_id,
            note: self.note.clone(),
            order_number: self.order_number(),
            paid_at: self.paid_at,
        })
    }

    pub fn items_for_display(
        &self,
        conn: &PgConnection,
    ) -> Result<Vec<DisplayOrderItem>, DatabaseError> {
        OrderItem::find_for_display(self.id, conn)
    }

    pub fn find_item(
        &self,
        cart_item_id: Uuid,
        conn: &PgConnection,
    ) -> Result<OrderItem, DatabaseError> {
        OrderItem::find_in_order(self.id, cart_item_id, conn)
    }

    pub fn find_item_by_type(
        &self,
        ticket_type_id: Uuid,
        item_type: OrderItemTypes,
        conn: &PgConnection,
    ) -> Result<OrderItem, DatabaseError> {
        let items = self.items(conn)?;
        let mut order_item: Vec<OrderItem> = items
            .into_iter()
            .filter(|i| i.ticket_type_id == Some(ticket_type_id) && i.item_type == item_type)
            .collect();

        match order_item.pop() {
            Some(o) => Ok(o),
            None => Err(DatabaseError::new(
                ErrorCode::NoResults,
                Some("Could not find item".to_string()),
            )),
        }
    }

    pub fn add_external_payment(
        &mut self,
        external_reference: Option<String>,
        current_user_id: Uuid,
        amount: i64,
        conn: &PgConnection,
    ) -> Result<Payment, DatabaseError> {
        let payment = Payment::create(
            self.id,
            current_user_id,
            PaymentStatus::Completed,
            PaymentMethods::External,
            "External".to_string(),
            external_reference,
            amount,
            None,
        );
        self.add_payment(payment, current_user_id, conn)
    }

    pub fn add_credit_card_payment(
        &mut self,
        current_user_id: Uuid,
        amount: i64,
        provider: String,
        external_reference: String,
        status: PaymentStatus,
        provider_data: serde_json::Value,
        conn: &PgConnection,
    ) -> Result<Payment, DatabaseError> {
        let payment = Payment::create(
            self.id,
            current_user_id,
            status,
            PaymentMethods::CreditCard,
            provider,
            Some(external_reference),
            amount,
            Some(provider_data),
        );

        self.add_payment(payment, current_user_id, conn)
    }

    pub fn add_pending_payment(
        &mut self,
        current_user_id: Uuid,
        amount: i64,
        provider: String,
        provider_data: Option<serde_json::Value>,
        conn: &PgConnection,
    ) -> Result<Payment, DatabaseError> {
        let payment = Payment::create(
            self.id,
            current_user_id,
            PaymentStatus::PendingCompletionAtGateway,
            PaymentMethods::Provider,
            provider,
            None,
            amount,
            provider_data,
        );
        self.add_payment(payment, current_user_id, conn)
    }

    fn add_payment(
        &mut self,
        payment: NewPayment,
        current_user_id: Uuid,
        conn: &PgConnection,
    ) -> Result<Payment, DatabaseError> {
        match self.status {
            OrderStatus::Paid => {
                return DatabaseError::business_process_error("This order has already been paid")
            }
            // orders can only expire if the order is in draft
            OrderStatus::Draft => self.mark_partially_paid(conn)?,
            OrderStatus::PartiallyPaid => (),
            _ => {
                return DatabaseError::business_process_error(&format!(
                    "Order was in unexpected state when trying to make a payment: {}",
                    self.status
                ))
            }
        }

        let p = payment.commit(current_user_id, conn)?;
        self.complete_if_fully_paid(conn)?;
        Ok(p)
    }

    fn mark_partially_paid(&mut self, conn: &PgConnection) -> Result<(), DatabaseError> {
        // TODO: The multiple queries in this method could probably be combined into a single query
        let now_plus_one_day = Utc::now().naive_utc() + Duration::days(1);
        let result = diesel::update(
            orders::table.filter(
                orders::id
                    .eq(self.id)
                    .and(orders::version.eq(self.version))
                    .and(orders::expires_at.ge(Some(Utc::now().naive_utc()))),
            ),
        )
        .set((
            orders::status.eq(OrderStatus::PartiallyPaid),
            orders::expires_at.eq(Some(now_plus_one_day)),
            orders::updated_at.eq(dsl::now),
        ))
        .execute(conn)
        .to_db_error(ErrorCode::UpdateError, "Could not update order status")?;

        let db_record = Order::find(self.id, conn)?;

        if result == 0 {
            if db_record.version != self.version {
                return DatabaseError::concurrency_error(
                    "Could not update order because it has been updated by another process",
                );
            }

            // Unfortunately, it's quite hard to work out what the current dsl::now() time is
            // So assume that the order has expired.
            return DatabaseError::business_process_error(
                "Could not update order because it has expired",
            );
        }

        self.updated_at = db_record.updated_at;
        self.status = db_record.status;

        //Extend the reserved_until time for tickets associated with this order
        let order_items = OrderItem::find_for_order(db_record.id, conn)?;

        for item in &order_items {
            TicketInstance::update_reserved_time(item, now_plus_one_day, conn)?;
        }

        Ok(())
    }

    pub(crate) fn complete_if_fully_paid(
        &mut self,
        conn: &PgConnection,
    ) -> Result<(), DatabaseError> {
        if self.total_paid(conn)? >= self.calculate_total(conn)? {
            self.update_status(OrderStatus::Paid, conn)?;
            //Mark tickets as Purchased
            let order_items = OrderItem::find_for_order(self.id, conn)?;
            for item in &order_items {
                TicketInstance::mark_as_purchased(item, self.user_id, conn)?;
            }
            let cart_user: Option<User> = users::table
                .filter(users::last_cart_id.eq(self.id))
                .get_result(conn)
                .to_db_error(
                    ErrorCode::QueryError,
                    "Could not find user attached to this cart",
                )
                .optional()?;

            if let Some(user) = cart_user {
                user.update_last_cart(None, conn)?;
            }
        }
        Ok(())
    }

    pub fn total_paid(&self, conn: &PgConnection) -> Result<i64, DatabaseError> {
        #[derive(QueryableByName)]
        struct ResultForSum {
            #[sql_type = "Nullable<BigInt>"]
            s: Option<i64>,
        };
        let query = diesel::sql_query(
            "SELECT CAST(SUM(amount) as BigInt) as s FROM payments WHERE order_id = $1 AND status='Completed';",
        )
        .bind::<diesel::sql_types::Uuid, _>(self.id);

        let sum: ResultForSum = query.get_result(conn).to_db_error(
            ErrorCode::QueryError,
            "Could not get total payments for order",
        )?;
        Ok(sum.s.unwrap_or(0))
    }

    fn update_box_office_pricing(
        &mut self,
        box_office_pricing: bool,
        conn: &PgConnection,
    ) -> Result<(), DatabaseError> {
        self.box_office_pricing = box_office_pricing;
        diesel::update(&*self)
            .set((
                orders::box_office_pricing.eq(&self.box_office_pricing),
                orders::updated_at.eq(dsl::now),
            ))
            .execute(conn)
            .to_db_error(ErrorCode::UpdateError, "Could not update order")?;

        Ok(())
    }

    fn update_status(
        &mut self,
        status: OrderStatus,
        conn: &PgConnection,
    ) -> Result<(), DatabaseError> {
        self.status = status;

        if status == OrderStatus::Paid {
            self.paid_at = Some(Utc::now().naive_utc());
            diesel::update(&*self)
                .set((
                    orders::paid_at.eq(self.paid_at),
                    orders::status.eq(&self.status),
                    orders::updated_at.eq(dsl::now),
                ))
                .execute(conn)
                .to_db_error(ErrorCode::UpdateError, "Could not mark order paid")?;
        } else {
            diesel::update(&*self)
                .set((
                    orders::status.eq(&self.status),
                    orders::updated_at.eq(dsl::now),
                ))
                .execute(conn)
                .to_db_error(ErrorCode::UpdateError, "Could not update order status")?;
        }

        Ok(())
    }

    pub fn calculate_total(&self, conn: &PgConnection) -> Result<i64, DatabaseError> {
        let order_items = self.items(conn)?;
        let mut total = 0;

        for item in &order_items {
            total += item.unit_price_in_cents * (item.quantity - item.refunded_quantity);
        }

        Ok(total)
    }

    /// Updates the lock version in the database and forces a Concurrency error if
    /// another process has updated it
    pub fn lock_version(&mut self, conn: &PgConnection) -> Result<(), DatabaseError> {
        let rows_affected = diesel::update(
            orders::table
                .filter(orders::id.eq(self.id))
                .filter(orders::version.eq(self.version)),
        )
        .set((
            orders::version.eq(self.version + 1),
            orders::updated_at.eq(dsl::now),
        ))
        .execute(conn)
        .to_db_error(ErrorCode::UpdateError, "Could not lock order")?;
        if rows_affected == 0 {
            return DatabaseError::concurrency_error(
                "Could not lock order, another process has updated it",
            );
        }
        self.version = self.version + 1;
        Ok(())
    }

    pub fn set_behalf_of_user(
        &mut self,
        user: User,
        current_user_id: Uuid,
        conn: &PgConnection,
    ) -> Result<(), DatabaseError> {
        if self.status != OrderStatus::Draft {
            return DatabaseError::validation_error(
                "status",
                "Cannot change the order user unless the order is in draft status",
            );
        }

        self.lock_version(conn)?;

        let old_id = self.on_behalf_of_user_id;
        self.on_behalf_of_user_id = Some(user.id);
        diesel::update(&*self)
            .set((
                orders::on_behalf_of_user_id.eq(user.id),
                orders::updated_at.eq(dsl::now),
            ))
            .execute(conn)
            .to_db_error(
                ErrorCode::UpdateError,
                "Could not change the behalf of  user for this order",
            )?;

        DomainEvent::create(
            DomainEventTypes::OrderBehalfOfUserChanged,
            "Behalf of user on order was changed".to_string(),
            Tables::Orders,
            Some(self.id),
            Some(current_user_id),
            Some(json!({
            "old_user" : old_id, "new_user": user.id
            })),
        )
        .commit(conn)?;
        Ok(())
    }
}

#[derive(QueryableByName, Deserialize, Serialize, Debug)]
pub struct ResultForTicketTypeTotal {
    #[sql_type = "Nullable<dUuid>"]
    ticket_type_id: Option<Uuid>,
    #[sql_type = "Integer"]
    total_quantity: i32,
}

#[derive(Deserialize, Serialize)]
pub struct TicketsRemaining {
    pub ticket_type_id: Uuid,
    pub tickets_remaining: i32,
}

#[derive(Deserialize, Serialize)]
pub struct DisplayOrder {
    pub id: Uuid,
    pub date: NaiveDateTime,
    pub expires_at: Option<NaiveDateTime>,
    pub seconds_until_expiry: Option<u32>,
    pub status: OrderStatus,
    pub items: Vec<DisplayOrderItem>,
    pub limited_tickets_remaining: Vec<TicketsRemaining>,
    pub total_in_cents: i64,
    pub user_id: Uuid,
    pub note: Option<String>,
    pub order_number: String,
    pub paid_at: Option<NaiveDateTime>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct UpdateOrderItem {
    pub ticket_type_id: Uuid,
    pub quantity: u32,
    pub redemption_code: Option<String>,
}

#[test]
fn parse_order_number() {
    let id = Uuid::parse_str("01234567-1234-1234-1234-1234567890ab").unwrap();
    assert_eq!("567890ab".to_string(), Order::parse_order_number(id));
}
