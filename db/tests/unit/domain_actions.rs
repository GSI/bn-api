use bigneon_db::dev::TestProject;
use bigneon_db::prelude::*;
use chrono::{Duration, Utc};
use uuid::Uuid;

#[test]
fn has_pending_action() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let id = Uuid::new_v4();

    // Empty, no action
    let result = DomainAction::has_pending_action(
        DomainActionTypes::MarketingContactsBulkEventFanListImport,
        Tables::Events.table_name(),
        id,
        connection,
    )
    .unwrap();
    assert!(!result);

    // New action
    let _domain_action = DomainAction::create(
        None,
        DomainActionTypes::MarketingContactsBulkEventFanListImport,
        None,
        json!(Vec::<u8>::new()),
        Some(Tables::Events.table_name()),
        Some(id),
        Utc::now()
            .naive_utc()
            .checked_add_signed(Duration::hours(1))
            .unwrap(),
        Utc::now()
            .naive_utc()
            .checked_add_signed(Duration::days(1))
            .unwrap(),
        3,
    )
    .commit(connection)
    .unwrap();

    let result = DomainAction::has_pending_action(
        DomainActionTypes::MarketingContactsBulkEventFanListImport,
        Tables::Events.table_name(),
        id,
        connection,
    )
    .unwrap();
    assert!(result);
}
