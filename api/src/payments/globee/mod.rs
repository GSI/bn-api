use globee::*;
use payments::charge_auth_result::ChargeAuthResult;
use payments::charge_result::ChargeResult;
use payments::payment_processor::PaymentProcessor;
use payments::payment_processor::PaymentProcessorBehavior;
use payments::payment_processor::RedirectInfo;
use payments::payment_processor::RedirectToPaymentPageBehavior;
use payments::payment_processor_error::PaymentProcessorError;
use payments::repeat_charge_token::RepeatChargeToken;

pub struct GlobeePaymentProcessor {
    api_key: String,
    base_url: String,
}

impl GlobeePaymentProcessor {
    pub fn new(api_key: String, base_url: String) -> GlobeePaymentProcessor {
        GlobeePaymentProcessor { api_key, base_url }
    }
}

impl PaymentProcessor for GlobeePaymentProcessor {
    fn behavior(&self) -> PaymentProcessorBehavior {
        PaymentProcessorBehavior::RedirectToPaymentPage(Box::new(GlobeePaymentProcessorBehavior {
            client: GlobeeClient::new(self.api_key.clone(), self.base_url.clone()),
        }))
    }

    fn refund(&self, auth_token: &str) -> Result<ChargeAuthResult, PaymentProcessorError> {
        unimplemented!()
    }

    fn partial_refund(
        &self,
        auth_token: &str,
        amount: u32,
    ) -> Result<ChargeAuthResult, PaymentProcessorError> {
        unimplemented!()
    }
}

pub struct GlobeePaymentProcessorBehavior {
    client: GlobeeClient,
}

impl RedirectToPaymentPageBehavior for GlobeePaymentProcessorBehavior {
    fn name(&self) -> String {
        "Globee".to_string()
    }

    fn create_payment_request(
        &self,
        amount: f64,
        email: String,
    ) -> Result<RedirectInfo, PaymentProcessorError> {
        let payment_request = PaymentRequest::new(amount, email);
        let result = self.client.create_payment_request(payment_request)?;
        Ok(RedirectInfo {
            id: result.id,
            redirect_url: result.redirect_url,
        })
    }
}

impl From<GlobeeError> for PaymentProcessorError {
    fn from(g: GlobeeError) -> Self {
        PaymentProcessorError {
            description: g.to_string(),
            cause: Some(Box::new(g)),
        }
    }
}
