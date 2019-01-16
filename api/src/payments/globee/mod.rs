use payments::charge_auth_result::ChargeAuthResult;
use payments::charge_result::ChargeResult;
use payments::payment_processor::PaymentProcessor;
use payments::payment_processor_error::PaymentProcessorError;
use payments::repeat_charge_token::RepeatChargeToken;

pub struct GlobeePaymentProcessor {
    key: String,
    secret: String,
}

impl GlobeePaymentProcessor {
    pub fn new(key: String, secret: String) -> GlobeePaymentProcessor {
        GlobeePaymentProcessor { key, secret }
    }
}
