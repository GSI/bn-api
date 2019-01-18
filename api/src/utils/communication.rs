use std::collections::HashMap;

use chrono::{Duration, Utc};
use diesel::PgConnection;
use futures::Future;
use tokio::prelude::*;
use uuid::Uuid;

use bigneon_db::models::enums::*;
use bigneon_db::models::*;
use config::{Config, Environment};
use errors::*;
use futures::future::Either;
use utils::sendgrid::mail as sendgrid;
use utils::twilio;

pub type TemplateData = HashMap<String, String>;

#[derive(Serialize, Deserialize)]
pub enum CommunicationType {
    Email,
    EmailTemplate,
    Sms,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct CommAddress {
    addresses: Vec<String>,
}

impl CommAddress {
    pub fn new() -> CommAddress {
        CommAddress {
            addresses: Vec::new(),
        }
    }

    pub fn from(address: String) -> CommAddress {
        CommAddress {
            addresses: vec![address],
        }
    }

    pub fn from_vec(addresses: Vec<String>) -> CommAddress {
        CommAddress { addresses }
    }

    pub fn get(&self) -> Vec<String> {
        (self.addresses.clone())
    }

    pub fn get_first(&self) -> Result<String, BigNeonError> {
        if self.addresses.len() >= 1 {
            Ok(self.addresses[0].clone())
        } else {
            Err(
                ApplicationError::new("Minimum of one communication address required".to_string())
                    .into(),
            )
        }
    }

    pub fn push(&mut self, address: &String) {
        self.addresses.push(address.clone());
    }
}
#[derive(Serialize, Deserialize)]
pub struct Communication {
    pub comm_type: CommunicationType,
    pub title: String,
    pub body: Option<String>,
    pub source: Option<CommAddress>,
    pub destinations: CommAddress,
    pub template_id: Option<String>,
    pub template_data: Option<Vec<TemplateData>>,
}

impl Communication {
    pub fn new(
        comm_type: CommunicationType,
        title: String,
        body: Option<String>,
        source: Option<CommAddress>,
        destinations: CommAddress,
        template_id: Option<String>,
        template_data: Option<Vec<TemplateData>>,
    ) -> Communication {
        Communication {
            comm_type,
            title,
            body,
            source,
            destinations,
            template_id,
            template_data,
        }
    }

    pub fn queue(&self, connection: &PgConnection) -> Result<(), BigNeonError> {
        DomainAction::create(
            None,
            DomainActionTypes::Communication,
            match self.comm_type {
                CommunicationType::Email => Some(CommunicationChannelType::Email),
                CommunicationType::EmailTemplate => Some(CommunicationChannelType::Email),
                CommunicationType::Sms => Some(CommunicationChannelType::Sms),
            },
            json!(self),
            None,
            None,
            Utc::now().naive_utc(),
            (Utc::now().naive_utc())
                .checked_add_signed(Duration::days(1))
                .unwrap(),
            3,
        )
        .commit(connection)?;
        Ok(())
    }

    pub fn send_async(
        domain_action: &DomainAction,
        config: &Config,
    ) -> impl Future<Item = (), Error = BigNeonError> {
        let communication: Communication =
            match serde_json::from_value(domain_action.payload.clone()) {
                Ok(v) => v,
                Err(e) => return Either::A(future::err(e.into())),
            };
        match config.environment {
            //TODO Maybe remove this environment check and just rely on the BLOCK_EXTERNAL_COMMS .env
            Environment::Test => Either::A(future::ok(())), //Disable communication system when testing
            _ => {
                let res = match config.block_external_comms {
                    true => Either::A(future::ok(())), //Disable communication system when block_external_comms is true,
                    _ => {
                        let destination_addresses = communication.destinations.get();
                        let source_address =
                            communication.source.as_ref().unwrap().get_first().unwrap();

                        let future = match communication.comm_type {
                            CommunicationType::Email => sendgrid::send_email_async(
                                &config.sendgrid_api_key,
                                source_address,
                                destination_addresses,
                                communication.title.clone(),
                                communication.body.clone(),
                            ),
                            CommunicationType::EmailTemplate => {
                                sendgrid::send_email_template_async(
                                    &config.sendgrid_api_key,
                                    source_address,
                                    &destination_addresses,
                                    communication.template_id.clone().unwrap(),
                                    communication.template_data.as_ref().unwrap(),
                                )
                            }
                            CommunicationType::Sms => twilio::send_sms_async(
                                &config.twilio_account_id,
                                &config.twilio_api_key,
                                source_address,
                                destination_addresses,
                                &communication.body.unwrap_or(communication.title),
                            ),
                        };
                        Either::B(future)
                    }
                };
                res
            }
        }
    }
}
