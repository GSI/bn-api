//#![deny(unreachable_patterns)]
//#![deny(unused_variables)]
//#![deny(unused_imports)]
//// Unused results is more often than not an error
//#![deny(unused_must_use)]
#[macro_use]
extern crate derive_error;
extern crate reqwest;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate url;

use std::error::Error as StdError;
use std::fmt;
use url::Url;

pub struct GlobeeClient {
    key: String,
    secret: String,
    base_url: String,
}

impl GlobeeClient {
    /// Creates a new Globee client
    /// base_url: Live: https://globee.com/payment-api/v1/, test: https://test.globee.com/payment-api/v1/
    pub fn new(key: String, secret: String, base_url: String) -> GlobeeClient {
        GlobeeClient {
            key,
            secret,
            base_url,
        }
    }

    pub fn create_payment_request(
        &self,
        request: PaymentRequest,
    ) -> Result<PaymentResponse, GlobeeError> {
        let client = reqwest::Client::new();
        let mut resp = client.post(&self.base_url).send()?;
        let value: GlobeeResponse<PaymentResponse> = resp.json()?;

        if value.success {
            match value.data {
                Some(data) => Ok(data),
                None => Err(GlobeeError::UnexpectedResponseError(
                    "API did not return a response that was expected".to_string(),
                )),
            }
        } else {
            match value.errors {
                Some(errors) => Err(GlobeeError::ValidationError(Errors(errors))),
                None => Err(GlobeeError::UnexpectedResponseError(
                    "API did not return a response that was expected".to_string(),
                )),
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum GlobeeError {
    ValidationError(Errors),
    HttpError(reqwest::Error),
    #[error(msg_embedded, no_from, non_std)]
    UnexpectedResponseError(String),
}

#[derive(Deserialize)]
struct GlobeeResponse<T> {
    success: bool,
    data: Option<T>,
    errors: Option<Vec<ValidationError>>,
}

#[derive(Serialize, Deserialize)]
pub struct PaymentRequest {
    /// The total amount in the invoice currency.
    // TODO: Replace with numeric type
    pub total: String,
    pub currency: Option<String>,
    /// A reference or custom identifier that you can use to link the payment back to your system.
    pub custom_payment_id: Option<String>,
    /// Passthrough data that will be returned in the IPN callback.
    pub callback_data: Option<String>,
    /// The customer making the payment
    pub customer: Customer,
    pub success_url: Option<String>,
    pub cancel_url: Option<String>,
    pub ipn_url: Option<String>,
    pub notification_email: Option<Email>,
    pub confirmation_speed: Option<ConfirmationSpeed>,
    pub custom_store_reference: Option<String>,
}

#[derive(Deserialize)]
pub struct PaymentResponse {
    pub id: String,
    pub status: String,
    pub adjusted_total: Option<f64>,
    #[serde(flatten)]
    pub request: PaymentRequest,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfirmationSpeed {
    High,
    Medium,
    Low,
}

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct Email(String);

#[derive(Serialize, Deserialize)]
pub struct Customer {
    pub name: Option<String>,
    pub email: Email,
}

#[derive(Deserialize, Debug)]
pub struct Errors(Vec<ValidationError>);

impl StdError for Errors {
    fn description(&self) -> &str {
        "One or more errors occurred"
    }
}

impl fmt::Display for Errors {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

#[derive(Deserialize, Debug)]
pub struct ValidationError {
    #[serde(rename = "type")]
    pub type_: String,
    pub extra: Option<Vec<String>>,
    pub field: String,
    pub message: String,
}
//
//impl StdError for ValidationError {
//    fn description(&self) -> String {
//        "One or more errors occurred"
//    }
//}
//
//use std::fmt;
//
//impl fmt::Display for ValidationError {}

#[cfg(test)]
pub mod test {
    use super::*;

    #[test]
    pub fn deserialize_data() {
        let data = r#"
            {
          "success": true,
          "data": {
            "id": "a1B2c3D4e5F6g7H8i9J0kL",
            "status": "unpaid",
            "total": "123.45",
            "currency": "USD",
            "custom_payment_id": "742",
            "custom_store_reference": "abc",
            "callback_data": "example data",
            "customer": {
              "name": "John Smit",
              "email": "john.smit@hotmail.com"
            },
            "payment_details": {
              "currency": null
            },
            "redirect_url": "http:\/\/globee.com\/invoice\/a1B2c3D4e5F6g7H8i9J0kL",
            "success_url": "https:\/\/www.example.com/success",
            "cancel_url": "https:\/\/www.example.com/cancel",
            "ipn_url": "https:\/\/www.example.com/globee/ipn-callback",
            "notification_email": null,
            "confirmation_speed": "medium",
            "expires_at": "2018-01-25 12:31:04",
            "created_at": "2018-01-25 12:16:04"
          }
        }
        "#;
        let response: GlobeeResponse<PaymentResponse> = serde_json::from_str(data).unwrap();

        assert_eq!(response.data.as_ref().unwrap().id, "a1B2c3D4e5F6g7H8i9J0kL");
        assert_eq!(response.data.unwrap().status, "unpaid");
        assert!(response.success);

        assert!(response.errors.is_none());
    }

    #[test]
    pub fn deserialize_error() {
        let data = r#"
            {
              "success": false,
              "errors": [
                {
                  "type": "required_field",
                  "extra": null,
                  "field": "customer.email",
                  "message": "The customer email field is required."
                },
                {
                  "type": "invalid_number",
                  "extra": null,
                  "field": "total",
                  "message": "The total must be a number."
                },
                {
                  "type": "below_minimum",
                  "extra": [
                    "10"
                  ],
                  "field": "total",
                  "message": "The total must be at least 10."
                },
                {
                  "type": "invalid_selection",
                  "extra": [
                    "AFN",
                    "ALL",
                    "DZD",
                    "..."
                  ],
                  "field": "currency",
                  "message": "The selected currency is invalid."
                }
              ]
            }"#;
        let response: GlobeeResponse<PaymentResponse> = serde_json::from_str(data).unwrap();

        assert!(response.data.is_none());
        assert!(!response.success);

        let errors = response.errors.unwrap();
        assert_eq!(errors.len(), 4);
    }

}
