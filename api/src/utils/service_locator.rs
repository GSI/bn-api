use config::Config;
use errors::*;
use payments::globee::GlobeePaymentProcessor;
use payments::stripe::StripePaymentProcessor;
use payments::PaymentProcessor;

pub struct ServiceLocator {
    stripe_secret_key: String,
    globee_api_key: String,
    globee_base_url: String,
}

impl ServiceLocator {
    pub fn new(config: &Config) -> ServiceLocator {
        ServiceLocator {
            stripe_secret_key: config.stripe_secret_key.to_string(),
            globee_api_key: config.globee_api_key.to_string(),
            globee_base_url: config.globee_base_url.to_string(),
        }
    }

    pub fn create_payment_processor(
        &self,
        provider_name: &str,
    ) -> Result<Box<PaymentProcessor>, BigNeonError> {
        match provider_name {
            "stripe" => Ok(Box::new(StripePaymentProcessor::new(
                self.stripe_secret_key.clone(),
            ))),
            "globee" => Ok(Box::new(GlobeePaymentProcessor::new(
                self.globee_api_key.clone(),
                self.globee_base_url.clone(),
            ))),
            _ => return Err(ApplicationError::new("Unknown payment provider".into()).into()),
        }
    }
}
