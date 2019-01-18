use bigneon_db::prelude::*;
use db::Connection;
use domain_events::executor_future::ExecutorFuture;
use domain_events::routing::DomainActionExecutor;
use errors::BigNeonError;
use futures::future;
use globee::GlobeeIpnRequest;
use uuid::Uuid;

pub struct ProcessPaymentIPNExecutor {}

impl DomainActionExecutor for ProcessPaymentIPNExecutor {
    fn execute(&self, action: DomainAction, conn: Connection) -> ExecutorFuture {
        match self.perform_job(&action, &conn) {
            Ok(_) => ExecutorFuture::new(action, conn, Box::new(future::ok(()))),
            Err(e) => ExecutorFuture::new(action, conn, Box::new(future::err(e))),
        }
    }
}

impl ProcessPaymentIPNExecutor {
    pub fn new() -> ProcessPaymentIPNExecutor {
        ProcessPaymentIPNExecutor {}
    }

    fn perform_job(&self, action: &DomainAction, conn: &Connection) -> Result<(), BigNeonError> {
        let ipn: GlobeeIpnRequest = serde_json::from_value(action.payload.clone())?;
        if ipn.custom_payment_id.is_none() {
            // TODO: Return failed?
            return Ok(());
        }
        let payment_id = Uuid::parse_str(ipn.custom_payment_id.as_ref().unwrap())?;
        let connection = conn.get();
        let payment = Payment::find(payment_id, connection)?;
        if ipn.status.as_ref().unwrap() == "paid" {
            payment.mark_complete(json!(ipn), None, connection)?;
        } else {
            payment.add_ipn(json!(ipn), None, connection)?;
        }

        Ok(())
    }
}
