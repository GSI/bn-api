use bigneon_db::prelude::*;
use chrono::{Duration, Utc};
use config::Config;
use db::Connection;
use diesel::PgConnection;
use domain_events::executor_future::ExecutorFuture;
use domain_events::routing::DomainActionExecutor;
use errors::{ApplicationError, BigNeonError};
use futures::future;
use log::Level::*;
use std::default::Default;
use utils::marketing_contacts::BulkEventFanListImportAction;
use utils::sendgrid::contacts::{SGContact, SGContactList};
use uuid::Uuid;

const LOG_TARGET: &'static str = "bigneon::domain_actions::marketing_contacts";
const HOURS_DELAY: i64 = 12;

pub struct BulkEventFanListImportExecutor {
    config: Config,
}

impl BulkEventFanListImportExecutor {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

#[derive(Serialize, Default, Deserialize, Clone, Debug)]
pub struct BulkEventFanListImportPayload {
    pub event_id: Uuid,
    #[serde(default)]
    pub execution_count: u32,
}

impl BulkEventFanListImportPayload {
    pub fn new(event_id: Uuid) -> Self {
        Self {
            event_id,
            ..Default::default()
        }
    }
}

impl DomainActionExecutor for BulkEventFanListImportExecutor {
    fn execute(&self, action: DomainAction, conn: Connection) -> ExecutorFuture {
        match self.perform_job(&action, &conn) {
            Ok(_) => ExecutorFuture::new(action, conn, Box::new(future::ok(()))),
            Err(e) => {
                jlog!(
                    Error,
                    LOG_TARGET,
                    "Error in BulkEventFanListImportExecutor",
                    { "innerError": format!("{}", e) }
                );
                ExecutorFuture::new(action, conn, Box::new(future::err(e)))
            }
        }
    }
}

impl BulkEventFanListImportExecutor {
    fn perform_job(
        &self,
        action: &DomainAction,
        connection: &Connection,
    ) -> Result<(), BigNeonError> {
        let payload =
            serde_json::from_value::<BulkEventFanListImportPayload>(action.payload.clone())?;

        jlog!(
            Info,
            LOG_TARGET,
            &format!(
                "Bulk fan list import task starting for event_id={}, execution_count={}",
                payload.event_id, payload.execution_count
            ), {"event_id" : payload.event_id, "execution_count": payload.execution_count, "action_id" : action.id}
        );

        let conn = connection.get();

        let event = Event::find(payload.event_id, conn)?;

        let mut org = event.organization(conn)?;

        org.decrypt(&self.config.api_keys_encryption_key)?;

        let api_key = match org.sendgrid_api_key {
            Some(v) => v,
            None => {
                jlog!(Info, LOG_TARGET, &format!("No sendgrid api key for org {}", event.organization_id), { "event_id": event.id });
                return Ok(());
            }
        };

        let (fans, total) = event.search_fans(None, None, None, None, None, conn)?;

        if total > 0 {
            let contacts = fans
                .into_iter()
                .filter(|fan| fan.email.is_some())
                .map(|fan| SGContact::new(fan.email.unwrap(), fan.first_name, fan.last_name))
                .collect::<Vec<SGContact>>();

            let result = SGContact::create_many(&api_key, contacts)?;

            if event.sendgrid_list_id.is_none() {
                return Err(ApplicationError::new(
                    "Event has no sendgrid list id. Cannot add recipients to list".to_string(),
                )
                .into());
            }

            let sg_list_id = event.sendgrid_list_id.unwrap();
            jlog!(Debug, LOG_TARGET, &format!("Fetching sendgrid list {}", sg_list_id), {
                "action_id": action.id,
                "sendgrid_list_id": sg_list_id,
                "event_id": event.id,
                "organization_id": event.organization_id,
            });

            let sg_list = SGContactList::get_by_id(&api_key, sg_list_id as u64)?;

            if !result.persisted_recipients.is_empty() {
                sg_list.add_recipients(&api_key, result.persisted_recipients)?;

                jlog!(Info, LOG_TARGET, &format!("Added {} recipients to sendgrid list '{}'", result.new_count, sg_list.id), {
                    "action_id": action.id,
                    "error_count": result.error_count,
                    // "error_emails": result.errors.iter().map(|e| e.error_indices).flatten().
                    "new_count": result.new_count,
                    "sendgrid_list_id": sg_list.id,
                    "event_id": event.id,
                    "organization_id": event.organization_id,
                });
            }
        }

        if self.is_event_on_sale(&event) {
            jlog!(Info, LOG_TARGET, &format!("Enqueuing delayed domain action for event={}", event.id), {
                "event_id": event.id,
                "action_id": action.id,
                "organization_id": event.organization_id,
            });
            self.enqueue_delayed_domain_action(conn, payload)?;
        } else {
            jlog!(Info, LOG_TARGET, &format!("Event {} is no longer on sale. Not enqueuing a new bulk fan list import task.", event.id), {
                "event_id": event.id,
                "action_id": action.id,
                "organization_id": event.organization_id,
            });
        }

        Ok(())
    }

    fn enqueue_delayed_domain_action(
        &self,
        conn: &PgConnection,
        payload: BulkEventFanListImportPayload,
    ) -> Result<DomainAction, BigNeonError> {
        let new_payload = BulkEventFanListImportPayload {
            execution_count: payload.execution_count + 1,
            ..payload
        };

        let scheduled_at = Utc::now()
            .naive_utc()
            .checked_add_signed(Duration::hours(HOURS_DELAY))
            .unwrap();

        BulkEventFanListImportAction::from_payload(new_payload)
            .enqueue_scheduled(conn, scheduled_at)
    }

    pub fn is_event_on_sale(&self, event: &Event) -> bool {
        let now = Utc::now().naive_utc();
        event.status == EventStatus::Published
            && (event.event_end.is_none() || event.event_end.unwrap() > now)
    }
}
