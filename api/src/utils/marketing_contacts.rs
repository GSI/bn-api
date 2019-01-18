use bigneon_db::models::enums::Tables;
use chrono::{Duration, NaiveDateTime, Utc};
use diesel::PgConnection;
use errors::BigNeonError;
use std::default::Default;
use uuid::Uuid;

use bigneon_db::models::enums::DomainActionTypes;
use bigneon_db::models::DomainAction;
use domain_events::executors::marketing_contacts::{
    BulkEventFanListImportPayload, CreateEventListPayload,
};

// This will be replaced once DomainEvents are functioning
pub struct CreateEventMarketingListAction {
    payload: CreateEventListPayload,
}

impl CreateEventMarketingListAction {
    pub fn new(event_id: Uuid) -> Self {
        Self::from_payload(CreateEventListPayload { event_id })
    }

    pub fn from_payload(payload: CreateEventListPayload) -> Self {
        Self { payload }
    }

    pub fn enqueue_scheduled(
        &self,
        connection: &PgConnection,
        scheduled_at: NaiveDateTime,
    ) -> Result<DomainAction, BigNeonError> {
        DomainAction::create(
            None,
            DomainActionTypes::MarketingContactsCreateEventList,
            None,
            json!(self.payload),
            Some(Tables::Events.table_name()),
            Some(self.payload.event_id),
            scheduled_at,
            scheduled_at.checked_add_signed(Duration::days(1)).unwrap(),
            3,
        )
        .commit(connection)
        .map_err(|err| BigNeonError::new(Box::new(err)))
    }

    pub fn enqueue(&self, connection: &PgConnection) -> Result<DomainAction, BigNeonError> {
        self.enqueue_scheduled(connection, Utc::now().naive_utc())
    }
}

pub struct BulkEventFanListImportAction {
    payload: BulkEventFanListImportPayload,
}

impl BulkEventFanListImportAction {
    pub fn new(event_id: Uuid) -> Self {
        Self::from_payload(BulkEventFanListImportPayload {
            event_id,
            ..Default::default()
        })
    }

    pub fn from_payload(payload: BulkEventFanListImportPayload) -> Self {
        Self { payload }
    }

    pub fn enqueue_scheduled(
        &self,
        connection: &PgConnection,
        scheduled_at: NaiveDateTime,
    ) -> Result<DomainAction, BigNeonError> {
        DomainAction::create(
            None,
            DomainActionTypes::MarketingContactsBulkEventFanListImport,
            None,
            json!(self.payload),
            Some(Tables::Events.table_name()),
            Some(self.payload.event_id),
            scheduled_at,
            scheduled_at.checked_add_signed(Duration::days(1)).unwrap(),
            3,
        )
        .commit(connection)
        .map_err(|err| BigNeonError::new(Box::new(err)))
    }

    pub fn enqueue(&self, connection: &PgConnection) -> Result<DomainAction, BigNeonError> {
        self.enqueue_scheduled(connection, Utc::now().naive_utc())
    }
}
