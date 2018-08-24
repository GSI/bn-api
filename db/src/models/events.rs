use chrono::NaiveDate;
use chrono::NaiveDateTime;
use db::Connectable;
use diesel;
use diesel::prelude::*;
use models::*;
use schema::{artists, event_artists, events, venues};
use utils::errors::DatabaseError;
use utils::errors::ErrorCode;
use uuid::Uuid;

#[derive(Associations, Identifiable, Queryable, AsChangeset)]
#[belongs_to(Organization)]
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[belongs_to(Venue)]
#[table_name = "events"]
pub struct Event {
    pub id: Uuid,
    pub name: String,
    pub organization_id: Uuid,
    pub venue_id: Uuid,
    pub created_at: NaiveDateTime,
    pub ticket_sell_date: NaiveDateTime,
    pub event_start: NaiveDateTime,
}

#[derive(Insertable, Serialize, Deserialize)]
#[table_name = "events"]
pub struct NewEvent {
    pub name: String,
    pub organization_id: Uuid,
    pub venue_id: Uuid,
    pub event_start: NaiveDateTime,
}

#[derive(AsChangeset, Default, Deserialize)]
#[table_name = "events"]
pub struct EventEditableAttributes {
    pub name: Option<String>,
    pub organization_id: Option<Uuid>,
    pub venue_id: Option<Uuid>,
    pub ticket_sell_date: Option<NaiveDateTime>,
    pub event_start: Option<NaiveDateTime>,
}

impl NewEvent {
    pub fn commit(&self, conn: &Connectable) -> Result<Event, DatabaseError> {
        DatabaseError::wrap(
            ErrorCode::InsertError,
            "Could not create new event",
            diesel::insert_into(events::table)
                .values(self)
                .get_result(conn.get_connection()),
        )
    }
}

impl Event {
    pub fn create(
        name: &str,
        organization_id: Uuid,
        venue_id: Uuid,
        event_start: NaiveDateTime,
    ) -> NewEvent {
        NewEvent {
            name: name.into(),
            organization_id: organization_id,
            venue_id: venue_id,
            event_start: event_start,
        }
    }

    pub fn update(
        &self,
        attributes: EventEditableAttributes,
        conn: &Connectable,
    ) -> Result<Event, DatabaseError> {
        DatabaseError::wrap(
            ErrorCode::UpdateError,
            "Could not update event",
            diesel::update(self)
                .set(attributes)
                .get_result(conn.get_connection()),
        )
    }

    pub fn find(id: Uuid, conn: &Connectable) -> Result<Event, DatabaseError> {
        DatabaseError::wrap(
            ErrorCode::QueryError,
            "Error loading event",
            events::table.find(id).first::<Event>(conn.get_connection()),
        )
    }

    pub fn find_all_events_from_venue(
        venue_id: &Uuid,
        conn: &Connectable,
    ) -> Result<Vec<Event>, DatabaseError> {
        DatabaseError::wrap(
            ErrorCode::QueryError,
            "Error loading event via venue",
            events::table
                .filter(events::venue_id.eq(venue_id))
                .load(conn.get_connection()),
        )
    }

    pub fn find_all_events_from_organization(
        organization_id: &Uuid,
        conn: &Connectable,
    ) -> Result<Vec<Event>, DatabaseError> {
        DatabaseError::wrap(
            ErrorCode::QueryError,
            "Error loading events via organization",
            events::table
                .filter(events::organization_id.eq(organization_id))
                .load(conn.get_connection()),
        )
    }

    pub fn search(
        query_filter: Option<String>,
        start_time: Option<NaiveDateTime>,
        end_time: Option<NaiveDateTime>,
        conn: &Connectable,
    ) -> Result<Vec<Event>, DatabaseError> {
        let query_like = match query_filter {
            Some(n) => format!("%{}%", n),
            None => "%".to_string(),
        };

        let result =
            events::table
                .filter(events::event_start.gt(
                    start_time.unwrap_or_else(|| NaiveDate::from_ymd(1970, 1, 1).and_hms(0, 0, 0)),
                ))
                .filter(events::event_start.lt(
                    end_time.unwrap_or_else(|| NaiveDate::from_ymd(3970, 1, 1).and_hms(0, 0, 0)),
                ))
                .left_join(
                    venues::table.on(events::venue_id
                        .eq(venues::id)
                        .and(venues::name.ilike(query_like.clone()))),
                )
                .left_join(
                    event_artists::table
                        .inner_join(
                            artists::table.on(event_artists::artist_id
                                .eq(artists::id)
                                .and(artists::name.ilike(query_like.clone()))),
                        )
                        .on(events::id.eq(event_artists::event_id)),
                )
                .filter(
                    events::name
                        .ilike(query_like.clone())
                        .or(venues::id.is_not_null())
                        .or(artists::id.is_not_null()),
                )
                .select(events::all_columns)
                .distinct()
                .order_by(events::event_start.asc())
                .then_order_by(events::name.asc())
                .load(conn.get_connection());

        DatabaseError::wrap(ErrorCode::QueryError, "Unable to load all events", result)
    }

    pub fn add_artist(&self, artist_id: Uuid, conn: &Connectable) -> Result<(), DatabaseError> {
        EventArtist::create(self.id, artist_id, 0)
            .commit(conn)
            .map(|_| ())
    }

    pub fn organization(&self, conn: &Connectable) -> Result<Organization, DatabaseError> {
        Organization::find(self.organization_id, conn)
    }

    pub fn add_ticket_allocation(
        &self,
        quantity: u32,
        conn: &Connectable,
    ) -> Result<TicketAllocation, DatabaseError> {
        TicketAllocation::create(self.id, quantity as i64).commit(conn)
    }
}