use payments::charge_auth_result::ChargeAuthResult;
use payments::charge_result::ChargeResult;
use payments::payment_processor_error::PaymentProcessorError;
use payments::repeat_charge_token::RepeatChargeToken;

pub enum PaymentProcessorBehavior {
    AuthThenComplete(Box<AuthThenCompletePaymentBehavior>),
    RedirectToPaymentPage,
}

pub trait AuthThenCompletePaymentBehavior {
    fn name(&self) -> String;

    fn create_token_for_repeat_charges(
        &self,
        token: &str,
        description: &str,
    ) -> Result<RepeatChargeToken, PaymentProcessorError>;

    fn update_repeat_token(
        &self,
        repeat_token: &str,
        token: &str,
        description: &str,
    ) -> Result<RepeatChargeToken, PaymentProcessorError>;

    fn auth(
        &self,
        token: &str,
        amount: i64,
        currency: &str,
        description: &str,
        metadata: Vec<(String, String)>,
    ) -> Result<ChargeAuthResult, PaymentProcessorError>;

    fn complete_authed_charge(
        &self,
        auth_token: &str,
    ) -> Result<ChargeResult, PaymentProcessorError>;
}

pub trait PaymentProcessor {
    fn behavior(&self) -> PaymentProcessorBehavior;
    fn refund(&self, auth_token: &str) -> Result<ChargeAuthResult, PaymentProcessorError>;

    fn partial_refund(
        &self,
        auth_token: &str,
        amount: u32,
    ) -> Result<ChargeAuthResult, PaymentProcessorError>;
}
