use chrono::prelude::*;
use diesel;
use diesel::dsl;
use diesel::prelude::*;
use models::*;
use schema::holds;
use utils::errors::*;
use uuid::Uuid;
use validator::Validate;
use validator::*;
use validators::{self, *};

#[derive(Clone, Deserialize, Identifiable, Queryable, Serialize, PartialEq, Debug)]
pub struct Hold {
    pub id: Uuid,
    pub name: String,
    pub parent_hold_id: Option<Uuid>,
    pub event_id: Uuid,
    pub redemption_code: String,
    pub discount_in_cents: Option<i64>,
    pub end_at: Option<NaiveDateTime>,
    pub max_per_order: Option<i64>,
    pub hold_type: HoldTypes,
    pub ticket_type_id: Uuid,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(AsChangeset, Default, Validate)]
#[table_name = "holds"]
pub struct UpdateHoldAttributes {
    pub name: Option<String>,
    pub redemption_code: Option<String>,
    pub hold_type: Option<HoldTypes>,
    pub discount_in_cents: Option<Option<i64>>,
    #[validate(email(message = "Email is invalid"))]
    pub email: Option<Option<String>>,
    pub phone: Option<Option<String>>,
    pub end_at: Option<Option<NaiveDateTime>>,
    pub max_per_order: Option<Option<i64>>,
}

impl Hold {
    /// Constructor for creating a simple hold that is accessible via a redemption code. For
    /// creating a comp, use `create_comp_for_person`.
    pub fn create_hold(
        name: String,
        event_id: Uuid,
        redemption_code: String,
        discount_in_cents: Option<u32>,
        end_at: Option<NaiveDateTime>,
        max_per_order: Option<u32>,
        hold_type: HoldTypes,
        ticket_type_id: Uuid,
    ) -> NewHold {
        NewHold {
            name,
            parent_hold_id: None,
            event_id,
            email: None,
            phone: None,
            redemption_code: redemption_code.to_uppercase(),
            discount_in_cents: discount_in_cents.and_then(|discount| Some(discount as i64)),
            end_at,
            max_per_order: max_per_order.map(|m| m as i64),
            hold_type,
            ticket_type_id,
        }
    }

    /// Creates a `quantity` of tickets in a hold (comp) for a person. `name` should
    /// be the name of the person, but does not have to be. For creating a simple hold,
    /// use `create_hold`.
    pub fn create_comp_for_person(
        name: String,
        hold_id: Uuid,
        email: Option<String>,
        phone: Option<String>,
        redemption_code: String,
        end_at: Option<NaiveDateTime>,
        max_per_order: Option<u32>,
        quantity: u32,
        conn: &PgConnection,
    ) -> Result<Hold, DatabaseError> {
        let hold = Hold::find(hold_id, conn)?;

        let new_hold = NewHold {
            name,
            parent_hold_id: Some(hold_id),
            event_id: hold.event_id,
            email,
            phone,
            redemption_code: redemption_code.to_uppercase(),
            discount_in_cents: None,
            end_at,
            max_per_order: max_per_order.map(|m| m as i64),
            hold_type: HoldTypes::Comp,
            ticket_type_id: hold.ticket_type_id,
        };

        let new_hold = new_hold.commit(conn)?;

        new_hold.set_quantity(quantity, conn)?;

        Ok(new_hold)
    }

    /// Updates a hold. Note, the quantity in the hold must be updated using
    /// `set_quantity`.
    pub fn update(
        &self,
        update_attrs: UpdateHoldAttributes,
        conn: &PgConnection,
    ) -> Result<Hold, DatabaseError> {
        let mut update_attrs = update_attrs;
        if update_attrs.hold_type == Some(HoldTypes::Comp) {
            // Remove discount
            update_attrs.discount_in_cents = Some(None);
        }

        self.validate_record(&update_attrs, conn)?;

        diesel::update(
            holds::table
                .filter(holds::id.eq(self.id))
                .filter(holds::updated_at.eq(self.updated_at)),
        )
        .set((update_attrs, holds::updated_at.eq(dsl::now)))
        .get_result(conn)
        .to_db_error(ErrorCode::UpdateError, "Could not update hold")
    }

    pub fn find(id: Uuid, conn: &PgConnection) -> Result<Hold, DatabaseError> {
        holds::table
            .filter(holds::id.eq(id))
            .first(conn)
            .to_db_error(ErrorCode::QueryError, "Could not retrieve hold")
    }

    pub fn find_by_parent_id(
        parent_id: Uuid,
        hold_type: HoldTypes,
        page: u32,
        limit: u32,
        conn: &PgConnection,
    ) -> Result<Payload<Hold>, DatabaseError> {
        let total: i64 = holds::table
            .filter(
                holds::hold_type
                    .eq(hold_type)
                    .and(holds::parent_hold_id.eq(parent_id)),
            )
            .count()
            .first(conn)
            .to_db_error(
                ErrorCode::QueryError,
                "Could not get total holds for parent hold",
            )?;

        let paging = Paging::new(page, limit);
        let mut payload = Payload::new(
            holds::table
                .filter(
                    holds::hold_type
                        .eq(hold_type)
                        .and(holds::parent_hold_id.eq(parent_id)),
                )
                .order_by(holds::name)
                .limit(limit as i64)
                .offset((page * limit) as i64)
                .load(conn)
                .to_db_error(ErrorCode::QueryError, "Could not retrieve holds")?,
            paging,
        );

        // TODO: remove this when other structs implement paging
        payload.paging.total = total as u64;
        payload.paging.page = page;
        payload.paging.limit = limit;
        Ok(payload)
    }

    pub fn find_for_event(event_id: Uuid, conn: &PgConnection) -> Result<Vec<Hold>, DatabaseError> {
        holds::table
            .filter(holds::event_id.eq(event_id))
            .order_by(holds::name.asc())
            .load(conn)
            .to_db_error(ErrorCode::QueryError, "Could not retrieve holds for event")
    }

    fn validate_record(
        &self,
        update_attrs: &UpdateHoldAttributes,
        conn: &PgConnection,
    ) -> Result<(), DatabaseError> {
        let validation_errors = validators::append_validation_error(
            update_attrs.validate(),
            "discount_in_cents",
            Hold::discount_in_cents_valid(
                update_attrs
                    .hold_type
                    .clone()
                    .unwrap_or(self.hold_type.clone()),
                update_attrs
                    .discount_in_cents
                    .unwrap_or(self.discount_in_cents),
            ),
        );
        let validation_errors = validators::append_validation_error(
            validation_errors,
            "redemption_code",
            redemption_code_unique_per_event_validation(
                Some(self.id),
                "holds".into(),
                update_attrs
                    .redemption_code
                    .clone()
                    .unwrap_or(self.redemption_code.clone()),
                conn,
            )?,
        );

        Ok(validation_errors?)
    }

    /// Creates a new hold, dividing the quantity in the calling hold (`self`). This is done so that
    /// there isn't a time where the tickets return to the main pool, or the parent hold and are
    /// reserved in between creation of the new hold. Note that the new hold created will have the
    /// same `parent_hold_id` as `self` (the original hold) and not `self`, as might be expected.
    /// If tickets are released from this new hold, or the original hold, they will be returned
    /// to the parent hold, being either the main pool or the parent hold in `parent_hold_id`.
    pub fn split(
        &self,
        name: String,
        redemption_code: String,
        quantity: u32,
        discount_in_cents: Option<u32>,
        hold_type: HoldTypes,
        end_at: Option<NaiveDateTime>,
        max_per_order: Option<u32>,
        conn: &PgConnection,
    ) -> Result<Hold, DatabaseError> {
        let new_hold = NewHold {
            name,
            parent_hold_id: self.parent_hold_id,
            event_id: self.event_id,
            email: None,
            phone: None,
            redemption_code: redemption_code.to_uppercase(),
            discount_in_cents: discount_in_cents.map(|m| m as i64),
            end_at,
            max_per_order: max_per_order.map(|m| m as i64),
            hold_type: hold_type,
            ticket_type_id: self.ticket_type_id,
        };

        let new_hold = new_hold.commit(conn)?;

        TicketInstance::add_to_hold(
            new_hold.id,
            self.ticket_type_id,
            quantity,
            Some(self.id),
            conn,
        )?;
        Ok(new_hold)
    }

    /// Deletes a hold by first setting the quantity to 0 and then deleting the record. If there
    /// are other holds that reference this hold via `parent_hold_id`, a `DatabaseError` with
    /// `ErrorCode::ForeignKeyError` will be returned.
    pub fn destroy(self, conn: &PgConnection) -> Result<(), DatabaseError> {
        self.set_quantity(0, conn)?;

        diesel::delete(holds::table.filter(holds::id.eq(self.id)))
            .execute(conn)
            .to_db_error(ErrorCode::DeleteError, "Could not delete hold")?;

        Ok(())
    }

    pub fn discount_in_cents_valid(
        hold_type: HoldTypes,
        discount_in_cents: Option<i64>,
    ) -> Result<(), ValidationError> {
        if hold_type == HoldTypes::Discount && discount_in_cents.is_none() {
            let validation_error =
                create_validation_error("required", "Discount required for hold type Discount");
            return Err(validation_error);
        }

        Ok(())
    }

    /// Changes the quantity of tickets reserved in this hold. If the quantity is
    /// higher, it will attempt to reserve more tickets from either the main pool,
    /// or from the parent hold if `parent_hold_id` is not `None`. Likewise, if the
    /// quantity is lower, it will release the reserved tickets back to either the
    /// main pool or the parent hold.
    pub fn set_quantity(&self, quantity: u32, conn: &PgConnection) -> Result<(), DatabaseError> {
        let (count, _available) = self.quantity(conn)?;
        if count < quantity {
            TicketInstance::add_to_hold(
                self.id,
                self.ticket_type_id,
                quantity - count,
                self.parent_hold_id,
                conn,
            )?;
        }
        if count > quantity {
            if self.parent_hold_id.is_some() {
                TicketInstance::add_to_hold(
                    self.parent_hold_id.unwrap(),
                    self.ticket_type_id,
                    count - quantity,
                    Some(self.id),
                    conn,
                )?;
            } else {
                TicketInstance::release_from_hold(
                    self.id,
                    self.ticket_type_id,
                    count - quantity,
                    conn,
                )?;
            }
        }
        Ok(())
    }

    pub fn quantity(&self, conn: &PgConnection) -> Result<(u32, u32), DatabaseError> {
        TicketInstance::count_for_hold(self.id, self.ticket_type_id, conn)
    }

    pub fn organization(&self, conn: &PgConnection) -> Result<Organization, DatabaseError> {
        use schema::*;
        events::table
            .inner_join(organizations::table)
            .filter(events::id.eq(self.event_id))
            .select(organizations::all_columns)
            .first(conn)
            .to_db_error(
                ErrorCode::QueryError,
                "Could not load organization for hold",
            )
    }

    pub fn find_by_redemption_code(
        redemption_code: &str,
        conn: &PgConnection,
    ) -> Result<Hold, DatabaseError> {
        holds::table
            .filter(holds::redemption_code.eq(redemption_code.to_uppercase()))
            .first(conn)
            .to_db_error(
                ErrorCode::QueryError,
                "Could not load hold with that redeem key",
            )
    }

    pub fn into_display(self, conn: &PgConnection) -> Result<DisplayHold, DatabaseError> {
        let (quantity, available) = self.quantity(conn)?;

        Ok(DisplayHold {
            id: self.id,
            name: self.name,
            parent_hold_id: self.parent_hold_id,
            event_id: self.event_id,
            redemption_code: self.redemption_code,
            discount_in_cents: self.discount_in_cents,
            max_per_order: self.max_per_order,
            email: self.email,
            phone: self.phone,
            available,
            quantity,
        })
    }

    pub fn comps(&self, conn: &PgConnection) -> Result<Vec<Hold>, DatabaseError> {
        Ok(Hold::find_by_parent_id(self.id, HoldTypes::Comp, 0, 100000, conn)?.data)
    }
}

#[derive(Insertable, Validate)]
#[table_name = "holds"]
pub struct NewHold {
    pub name: String,
    pub parent_hold_id: Option<Uuid>,
    pub event_id: Uuid,
    #[validate(email(message = "Email is invalid"))]
    pub email: Option<String>,
    pub phone: Option<String>,
    pub redemption_code: String,
    pub discount_in_cents: Option<i64>,
    pub end_at: Option<NaiveDateTime>,
    pub max_per_order: Option<i64>,
    pub hold_type: HoldTypes,
    pub ticket_type_id: Uuid,
}

impl NewHold {
    pub fn commit(mut self, conn: &PgConnection) -> Result<Hold, DatabaseError> {
        if self.hold_type == HoldTypes::Comp {
            self.discount_in_cents = None
        }
        self.validate_record(conn)?;
        diesel::insert_into(holds::table)
            .values(self)
            .get_result(conn)
            .to_db_error(ErrorCode::InsertError, "Could not create hold")
    }

    fn validate_record(&self, conn: &PgConnection) -> Result<(), DatabaseError> {
        let validation_errors = validators::append_validation_error(
            self.validate(),
            "discount_in_cents",
            Hold::discount_in_cents_valid(self.hold_type.clone(), self.discount_in_cents),
        );
        let validation_errors = validators::append_validation_error(
            validation_errors,
            "redemption_code",
            redemption_code_unique_per_event_validation(
                None,
                "holds".into(),
                self.redemption_code.clone(),
                conn,
            )?,
        );

        Ok(validation_errors?)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct DisplayHold {
    pub id: Uuid,
    pub parent_hold_id: Option<Uuid>,
    pub name: String,
    pub event_id: Uuid,
    pub redemption_code: String,
    pub discount_in_cents: Option<i64>,
    pub max_per_order: Option<i64>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub available: u32,
    pub quantity: u32,
}
