use payments::charge_auth_result::ChargeAuthResult;
use payments::charge_result::ChargeResult;
use payments::payment_processor::AuthThenCompletePaymentBehavior;
use payments::payment_processor::PaymentProcessor;
use payments::payment_processor::PaymentProcessorBehavior;
use payments::payment_processor_error::PaymentProcessorError;
use payments::repeat_charge_token::RepeatChargeToken;
use stripe::StripeClient;
use stripe::StripeError;

impl From<StripeError> for PaymentProcessorError {
    fn from(s: StripeError) -> PaymentProcessorError {
        PaymentProcessorError {
            description: s.description.clone(),
            cause: Some(Box::new(s)),
        }
    }
}

pub struct StripePaymentProcessor {
    client: StripeClient,
}

impl StripePaymentProcessor {
    pub fn new(stripe_secret_key: String) -> StripePaymentProcessor {
        StripePaymentProcessor {
            client: StripeClient::new(stripe_secret_key),
        }
    }
}

pub struct StripePaymentBehavior {
    client: StripeClient,
}

impl PaymentProcessor for StripePaymentProcessor {
    fn behavior(&self) -> PaymentProcessorBehavior {
        PaymentProcessorBehavior::AuthThenComplete(Box::new(StripePaymentBehavior {
            client: self.client.clone(),
        }))
    }

    fn refund(&self, auth_token: &str) -> Result<ChargeAuthResult, PaymentProcessorError> {
        Ok(self.client.refund(auth_token).map(|r| ChargeAuthResult {
            id: r.id,
            raw: r.raw_data,
        })?)
    }

    fn partial_refund(
        &self,
        auth_token: &str,
        amount: u32,
    ) -> Result<ChargeAuthResult, PaymentProcessorError> {
        Ok(self
            .client
            .partial_refund(auth_token, amount)
            .map(|r| ChargeAuthResult {
                id: r.id,
                raw: r.raw_data,
            })?)
    }
}

impl<'a> AuthThenCompletePaymentBehavior for StripePaymentBehavior {
    fn name(&self) -> String {
        "Stripe".to_string()
    }
    fn create_token_for_repeat_charges(
        &self,
        token: &str,
        description: &str,
    ) -> Result<RepeatChargeToken, PaymentProcessorError> {
        Ok(self
            .client
            .create_customer(description, token, Vec::<(String, String)>::new())
            .map(|r| RepeatChargeToken {
                token: r.id,
                raw: r.raw_data,
            })?)
    }

    fn update_repeat_token(
        &self,
        repeat_token: &str,
        token: &str,
        description: &str,
    ) -> Result<RepeatChargeToken, PaymentProcessorError> {
        Ok(self
            .client
            .update_customer(
                repeat_token,
                description,
                token,
                Vec::<(String, String)>::new(),
            )
            .map(|r| RepeatChargeToken {
                token: r.id,
                raw: r.raw_data,
            })?)
    }

    fn auth(
        &self,
        token: &str,
        amount: i64,
        currency: &str,
        description: &str,
        metadata: Vec<(String, String)>,
    ) -> Result<ChargeAuthResult, PaymentProcessorError> {
        Ok(self
            .client
            .auth(token, amount, currency, description, metadata)
            .map(|r| ChargeAuthResult {
                id: r.id,
                raw: r.raw_data,
            })?)
    }

    fn complete_authed_charge(
        &self,
        _auth_token: &str,
    ) -> Result<ChargeResult, PaymentProcessorError> {
        Ok(self.client.complete(_auth_token).map(|r| ChargeResult {
            id: r.id,
            raw: r.raw_data,
        })?)
    }
}
