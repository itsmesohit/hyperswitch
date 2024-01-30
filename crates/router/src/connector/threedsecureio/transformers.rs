use common_utils::date_time;
use error_stack::{report, IntoReport, ResultExt};
use iso_currency::Currency;
use isocountry;
use masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{
    connector::utils::{AddressDetailsData, CardData, PaymentsAuthorizeRequestData},
    core::errors,
    types::{
        self,
        api::{self, MessageCategory},
        storage::enums,
        transformers::ForeignTryFrom,
    },
};

//TODO: Fill the struct with respective fields
pub struct ThreedsecureioRouterData<T> {
    pub amount: i64, // The type of amount that a connector accepts, for example, String, i64, f64, etc.
    pub router_data: T,
}

impl<T>
    TryFrom<(
        &types::api::CurrencyUnit,
        types::storage::enums::Currency,
        i64,
        T,
    )> for ThreedsecureioRouterData<T>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        (_currency_unit, _currency, amount, item): (
            &types::api::CurrencyUnit,
            types::storage::enums::Currency,
            i64,
            T,
        ),
    ) -> Result<Self, Self::Error> {
        //Todo :  use utils to convert the amount to the type of amount that a connector accepts
        Ok(Self {
            amount,
            router_data: item,
        })
    }
}

impl<T> TryFrom<(i64, T)> for ThreedsecureioRouterData<T> {
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(router_data: (i64, T)) -> Result<Self, Self::Error> {
        //Todo :  use utils to convert the amount to the type of amount that a connector accepts
        Ok(Self {
            amount: router_data.0,
            router_data: router_data.1,
        })
    }
}

//TODO: Fill the struct with respective fields
#[derive(Default, Debug, Serialize, Eq, PartialEq)]
pub struct ThreedsecureioPaymentsRequest {
    amount: i64,
    card: ThreedsecureioCard,
}

#[derive(Default, Debug, Serialize, Eq, PartialEq)]
pub struct ThreedsecureioCard {
    number: cards::CardNumber,
    expiry_month: Secret<String>,
    expiry_year: Secret<String>,
    cvc: Secret<String>,
    complete: bool,
}

impl TryFrom<&ThreedsecureioRouterData<&types::PaymentsAuthorizeRouterData>>
    for ThreedsecureioPaymentsRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: &ThreedsecureioRouterData<&types::PaymentsAuthorizeRouterData>,
    ) -> Result<Self, Self::Error> {
        match item.router_data.request.payment_method_data.clone() {
            api::PaymentMethodData::Card(req_card) => {
                let card = ThreedsecureioCard {
                    number: req_card.card_number,
                    expiry_month: req_card.card_exp_month,
                    expiry_year: req_card.card_exp_year,
                    cvc: req_card.card_cvc,
                    complete: item.router_data.request.is_auto_capture()?,
                };
                Ok(Self {
                    amount: item.amount.to_owned(),
                    card,
                })
            }
            _ => Err(errors::ConnectorError::NotImplemented("Payment methods".to_string()).into()),
        }
    }
}

//TODO: Fill the struct with respective fields
// Auth Struct
pub struct ThreedsecureioAuthType {
    pub(super) api_key: Secret<String>,
}

impl TryFrom<&types::ConnectorAuthType> for ThreedsecureioAuthType {
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(auth_type: &types::ConnectorAuthType) -> Result<Self, Self::Error> {
        match auth_type {
            types::ConnectorAuthType::HeaderKey { api_key } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(errors::ConnectorError::FailedToObtainAuthType.into()),
        }
    }
}
// PaymentsResponse
//TODO: Append the remaining status flags
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ThreedsecureioPaymentStatus {
    Succeeded,
    Failed,
    #[default]
    Processing,
}

impl From<ThreedsecureioPaymentStatus> for enums::AttemptStatus {
    fn from(item: ThreedsecureioPaymentStatus) -> Self {
        match item {
            ThreedsecureioPaymentStatus::Succeeded => Self::Charged,
            ThreedsecureioPaymentStatus::Failed => Self::Failure,
            ThreedsecureioPaymentStatus::Processing => Self::Authorizing,
        }
    }
}

//TODO: Fill the struct with respective fields
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThreedsecureioPaymentsResponse {
    status: ThreedsecureioPaymentStatus,
    id: String,
}

impl<F, T>
    TryFrom<
        types::ResponseRouterData<
            F,
            ThreedsecureioPaymentsResponse,
            T,
            types::PaymentsResponseData,
        >,
    > for types::RouterData<F, T, types::PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: types::ResponseRouterData<
            F,
            ThreedsecureioPaymentsResponse,
            T,
            types::PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            status: enums::AttemptStatus::from(item.response.status),
            response: Ok(types::PaymentsResponseData::TransactionResponse {
                resource_id: types::ResponseId::ConnectorTransactionId(item.response.id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
            }),
            ..item.data
        })
    }
}

//TODO: Fill the struct with respective fields
// REFUND :
// Type definition for RefundRequest
#[derive(Default, Debug, Serialize)]
pub struct ThreedsecureioRefundRequest {
    pub amount: i64,
}

impl<F> TryFrom<&ThreedsecureioRouterData<&types::RefundsRouterData<F>>>
    for ThreedsecureioRefundRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: &ThreedsecureioRouterData<&types::RefundsRouterData<F>>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            amount: item.amount.to_owned(),
        })
    }
}

// Type definition for Refund Response

#[allow(dead_code)]
#[derive(Debug, Serialize, Default, Deserialize, Clone)]
pub enum RefundStatus {
    Succeeded,
    Failed,
    #[default]
    Processing,
}

impl From<RefundStatus> for enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            RefundStatus::Succeeded => Self::Success,
            RefundStatus::Failed => Self::Failure,
            RefundStatus::Processing => Self::Pending,
            //TODO: Review mapping
        }
    }
}

//TODO: Fill the struct with respective fields
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct RefundResponse {
    id: String,
    status: RefundStatus,
}

impl TryFrom<types::RefundsResponseRouterData<api::Execute, RefundResponse>>
    for types::RefundsRouterData<api::Execute>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: types::RefundsResponseRouterData<api::Execute, RefundResponse>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(types::RefundsResponseData {
                connector_refund_id: item.response.id.to_string(),
                refund_status: enums::RefundStatus::from(item.response.status),
            }),
            ..item.data
        })
    }
}

impl TryFrom<types::RefundsResponseRouterData<api::RSync, RefundResponse>>
    for types::RefundsRouterData<api::RSync>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: types::RefundsResponseRouterData<api::RSync, RefundResponse>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(types::RefundsResponseData {
                connector_refund_id: item.response.id.to_string(),
                refund_status: enums::RefundStatus::from(item.response.status),
            }),
            ..item.data
        })
    }
}

fn get_card_details(
    payment_method_data: api_models::payments::PaymentMethodData,
) -> Result<api_models::payments::Card, errors::ConnectorError> {
    match payment_method_data {
        api_models::payments::PaymentMethodData::Card(details) => Ok(details),
        _ => Err(errors::ConnectorError::RequestEncodingFailed)?,
    }
}

impl TryFrom<&ThreedsecureioRouterData<&types::ConnectorAuthenticationRouterData>>
    for ThreedsecureioAuthenticationRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: &ThreedsecureioRouterData<&types::ConnectorAuthenticationRouterData>,
    ) -> Result<Self, Self::Error> {
        let card_details = get_card_details(item.router_data.request.payment_method_data.clone())?;
        let currency = item
            .router_data
            .request
            .currency
            .map(|currency| currency.to_string())
            .ok_or(errors::ConnectorError::RequestEncodingFailed)?;
        let purchase_currency: Currency = iso_currency::Currency::from_code(&currency)
            .ok_or(errors::ConnectorError::RequestEncodingFailed)?;
        let address = item
            .router_data
            .request
            .billing_address
            .address
            .clone()
            .ok_or(errors::ConnectorError::RequestEncodingFailed)?;
        let billing_state = address.clone().to_state_code()?;
        let billing_country = isocountry::CountryCode::for_alpha2(
            &item
                .router_data
                .request
                .billing_address
                .address
                .clone()
                .and_then(|address| address.country)
                .ok_or(errors::ConnectorError::RequestEncodingFailed)?
                .to_string(),
        )
        .into_report()
        .change_context(errors::ConnectorError::RequestEncodingFailed)
        .attach_printable("Error parsing billing country type2")?;
        Ok(Self {
            ds_start_protocol_version: item
                .router_data
                .request
                .authentication_data
                .message_version
                .clone(),
            ds_end_protocol_version: item
                .router_data
                .request
                .authentication_data
                .message_version
                .clone(),
            acs_start_protocol_version: item
                .router_data
                .request
                .authentication_data
                .message_version
                .clone(),
            acs_end_protocol_version: item
                .router_data
                .request
                .authentication_data
                .message_version
                .clone(),
            three_dsserver_trans_id: item
                .router_data
                .request
                .authentication_data
                .threeds_server_transaction_id
                .clone(),
            acct_number: card_details.card_number.clone(),
            notification_url: "https://webhook.site/8d03e3ea-a7d8-48f5-a200-476bca75a55c"
                .to_string(),
            three_dscomp_ind: "Y".to_string(),
            three_dsrequestor_url: "https::/google.com".to_string(),
            acquirer_bin: item
                .router_data
                .request
                .acquirer_details
                .clone()
                .map(|acquirer| acquirer.acquirer_bin)
                .ok_or(errors::ConnectorError::RequestEncodingFailed)?,
            acquirer_merchant_id: item
                .router_data
                .request
                .acquirer_details
                .clone()
                .map(|acquirer| acquirer.acquirer_merchant_mid)
                .ok_or(errors::ConnectorError::RequestEncodingFailed)?,
            card_expiry_date: card_details.get_expiry_date_as_yymm()?.expose(),
            bill_addr_city: item
                .router_data
                .request
                .billing_address
                .address
                .clone()
                .and_then(|address| address.city)
                .ok_or(errors::ConnectorError::RequestEncodingFailed)?
                .to_string(),
            bill_addr_country: billing_country.numeric_id().to_string(),
            bill_addr_line1: item
                .router_data
                .request
                .billing_address
                .address
                .clone()
                .and_then(|address| address.line1)
                .ok_or(errors::ConnectorError::RequestEncodingFailed)?
                .expose()
                .to_string(),
            bill_addr_post_code: item
                .router_data
                .request
                .billing_address
                .address
                .clone()
                .and_then(|address| address.zip)
                .ok_or(errors::ConnectorError::RequestEncodingFailed)?
                .expose()
                .to_string(),
            bill_addr_state: billing_state.peek().to_string(),
            three_dsrequestor_authentication_ind: "01".to_string(),
            device_channel: item.router_data.request.device_channel.clone(),
            message_category: if item.router_data.request.message_category
                == MessageCategory::Payment
            {
                "01".to_string()
            } else {
                "02".to_string()
            },
            browser_javascript_enabled: item
                .router_data
                .request
                .browser_details
                .java_script_enabled
                .ok_or(errors::ConnectorError::RequestEncodingFailed)?,
            browser_accept_header: item
                .router_data
                .request
                .browser_details
                .accept_header
                .clone()
                .ok_or(errors::ConnectorError::RequestEncodingFailed)?,
            browser_ip: item
                .router_data
                .request
                .browser_details
                .ip_address
                .map(|ip| ip.to_string()),
            browser_java_enabled: item
                .router_data
                .request
                .browser_details
                .java_enabled
                .ok_or(errors::ConnectorError::RequestEncodingFailed)?,
            browser_language: item
                .router_data
                .request
                .browser_details
                .language
                .clone()
                .ok_or(errors::ConnectorError::RequestEncodingFailed)?,
            browser_color_depth: item
                .router_data
                .request
                .browser_details
                .color_depth
                .ok_or(errors::ConnectorError::RequestEncodingFailed)?
                .to_string(),
            browser_screen_height: item
                .router_data
                .request
                .browser_details
                .screen_height
                .ok_or(errors::ConnectorError::RequestEncodingFailed)?
                .to_string(),
            browser_screen_width: item
                .router_data
                .request
                .browser_details
                .screen_width
                .ok_or(errors::ConnectorError::RequestEncodingFailed)?
                .to_string(),
            browser_tz: item
                .router_data
                .request
                .browser_details
                .time_zone
                .ok_or(errors::ConnectorError::RequestEncodingFailed)?
                .to_string(),
            browser_user_agent: item
                .router_data
                .request
                .browser_details
                .user_agent
                .clone()
                .ok_or(errors::ConnectorError::RequestEncodingFailed)?
                .to_string(),
            mcc: "5411".to_string(),
            merchant_country_code: "840".to_string(),
            merchant_name: "Dummy Merchant".to_string(),
            message_type: "AReq".to_string(),
            message_version: item
                .router_data
                .request
                .authentication_data
                .message_version
                .clone(),
            purchase_amount: item.amount.to_string(),
            purchase_currency: purchase_currency.numeric().to_string(),
            trans_type: "01".to_string(),       //TODO
            purchase_exponent: "2".to_string(), //TODO
            purchase_date: date_time::DateTime::<date_time::YYYYMMDDHHmmss>::from(date_time::now())
                .to_string(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreedsecureioErrorResponse {
    pub error_code: String,
    pub error_component: String,
    pub error_description: String,
    pub error_detail: String,
    pub error_message_type: String,
    pub message_type: String,
    pub message_version: String,
    pub three_dsserver_trans_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreedsecureioAuthenticationResponse {
    #[serde(rename = "acsChallengeMandated")]
    pub acs_challenge_mandated: Option<String>,
    #[serde(rename = "acsOperatorID")]
    pub acs_operator_id: String,
    #[serde(rename = "acsReferenceNumber")]
    pub acs_reference_number: String,
    #[serde(rename = "acsTransID")]
    pub acs_trans_id: String,
    #[serde(rename = "acsURL")]
    pub acs_url: Option<url::Url>,
    #[serde(rename = "authenticationType")]
    pub authentication_type: Option<String>,
    #[serde(rename = "dsReferenceNumber")]
    pub ds_reference_number: String,
    #[serde(rename = "dsTransID")]
    pub ds_trans_id: String,
    #[serde(rename = "messageType")]
    pub message_type: Option<String>,
    #[serde(rename = "messageVersion")]
    pub message_version: String,
    #[serde(rename = "threeDSServerTransID")]
    pub three_dsserver_trans_id: String,
    #[serde(rename = "transStatus")]
    pub trans_status: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreedsecureioAuthenticationRequest {
    pub ds_start_protocol_version: String,
    pub ds_end_protocol_version: String,
    pub acs_start_protocol_version: String,
    pub acs_end_protocol_version: String,
    pub three_dsserver_trans_id: String,
    pub acct_number: cards::CardNumber,
    pub notification_url: String,
    pub three_dscomp_ind: String,
    pub three_dsrequestor_url: String,
    pub acquirer_bin: String,
    pub acquirer_merchant_id: String,
    pub card_expiry_date: String,
    pub bill_addr_city: String,
    pub bill_addr_country: String,
    pub bill_addr_line1: String,
    pub bill_addr_post_code: String,
    pub bill_addr_state: String,
    // pub email: Email,
    pub three_dsrequestor_authentication_ind: String,
    // pub cardholder_name: Secret<String>,
    pub device_channel: String,
    pub browser_javascript_enabled: bool,
    pub browser_accept_header: String,
    pub browser_ip: Option<String>,
    pub browser_java_enabled: bool,
    pub browser_language: String,
    pub browser_color_depth: String,
    pub browser_screen_height: String,
    pub browser_screen_width: String,
    pub browser_tz: String,
    pub browser_user_agent: String,
    pub mcc: String,
    pub merchant_country_code: String,
    pub merchant_name: String,
    pub message_category: String,
    pub message_type: String,
    pub message_version: String,
    pub purchase_amount: String,
    pub purchase_currency: String,
    pub purchase_exponent: String,
    pub purchase_date: String,
    pub trans_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreedsecureioPreAuthenticationRequest {
    acct_number: String,
    ds: Option<DirectoryServer>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DirectoryServer {
    Standin,
    Visa,
    Mastercard,
    Jcb,
    Upi,
    Amex,
    Protectbuy,
    Sbn,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreedsecureioPreAuthenticationResponse {
    pub ds_start_protocol_version: String,
    pub ds_end_protocol_version: String,
    pub acs_start_protocol_version: String,
    pub acs_end_protocol_version: String,
    #[serde(rename = "threeDSMethodURL")]
    pub threeds_method_url: Option<String>,
    #[serde(rename = "threeDSServerTransID")]
    pub threeds_server_trans_id: String,
    pub scheme: String,
    pub message_type: String,
}

impl TryFrom<&ThreedsecureioRouterData<&types::authentication::PreAuthNRouterData>>
    for ThreedsecureioPreAuthenticationRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        value: &ThreedsecureioRouterData<&types::authentication::PreAuthNRouterData>,
    ) -> Result<Self, Self::Error> {
        let router_data = value.router_data;
        Ok(Self {
            acct_number: router_data
                .request
                .card_holder_account_number
                .clone()
                .get_card_no(),
            ds: None,
        })
    }
}

impl ForeignTryFrom<String> for (i64, i64, i64) {
    type Error = error_stack::Report<errors::ConnectorError>;
    fn foreign_try_from(value: String) -> Result<Self, Self::Error> {
        let mut splitted_version = value.split('.');
        let version_string = {
            let major_version = splitted_version.next().ok_or(report!(
                errors::ConnectorError::ResponseDeserializationFailed
            ))?;
            let minor_version = splitted_version.next().ok_or(report!(
                errors::ConnectorError::ResponseDeserializationFailed
            ))?;
            let patch_version = splitted_version.next().ok_or(report!(
                errors::ConnectorError::ResponseDeserializationFailed
            ))?;
            (major_version, minor_version, patch_version)
        };
        let int_representation = {
            let major_version = version_string
                .0
                .parse()
                .into_report()
                .change_context(errors::ConnectorError::ResponseDeserializationFailed)?;
            let minor_version = version_string
                .1
                .parse()
                .into_report()
                .change_context(errors::ConnectorError::ResponseDeserializationFailed)?;
            let patch_version = version_string
                .2
                .parse()
                .into_report()
                .change_context(errors::ConnectorError::ResponseDeserializationFailed)?;
            (major_version, minor_version, patch_version)
        };
        Ok(int_representation)
    }
}