use desfire::desfire::desfire::MAX_BYTES_PER_TRANSACTION;
use desfire::desfire::Desfire;
use desfire::error::Error as DesfireError;
use desfire::iso7816_4::apduresponse::APDUResponse;
use rsasl::error::{MechanismError, MechanismErrorKind, SASLError, SessionError};
use rsasl::mechanism::Authentication;
use rsasl::property::AuthId;
use rsasl::session::{SessionData, StepResult};
use rsasl::SASL;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt::{Debug, Display, Formatter};
use std::io::Write;
use std::sync::Arc;

use crate::authentication::fabfire::FabFireCardKey;

enum FabFireError {
    ParseError,
    SerializationError,
    DeserializationError(serde_json::Error),
    CardError(DesfireError),
    InvalidMagic(String),
    InvalidToken(String),
    InvalidURN(String),
    InvalidCredentials(String),
    Session(SessionError),
}

impl Debug for FabFireError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FabFireError::ParseError => write!(f, "ParseError"),
            FabFireError::SerializationError => write!(f, "SerializationError"),
            FabFireError::DeserializationError(e) => write!(f, "DeserializationError: {}", e),
            FabFireError::CardError(err) => write!(f, "CardError: {}", err),
            FabFireError::InvalidMagic(magic) => write!(f, "InvalidMagic: {}", magic),
            FabFireError::InvalidToken(token) => write!(f, "InvalidToken: {}", token),
            FabFireError::InvalidURN(urn) => write!(f, "InvalidURN: {}", urn),
            FabFireError::InvalidCredentials(credentials) => {
                write!(f, "InvalidCredentials: {}", credentials)
            }
            FabFireError::Session(err) => write!(f, "Session: {}", err),
        }
    }
}

impl Display for FabFireError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FabFireError::ParseError => write!(f, "ParseError"),
            FabFireError::SerializationError => write!(f, "SerializationError"),
            FabFireError::DeserializationError(e) => write!(f, "DeserializationError: {}", e),
            FabFireError::CardError(err) => write!(f, "CardError: {}", err),
            FabFireError::InvalidMagic(magic) => write!(f, "InvalidMagic: {}", magic),
            FabFireError::InvalidToken(token) => write!(f, "InvalidToken: {}", token),
            FabFireError::InvalidURN(urn) => write!(f, "InvalidURN: {}", urn),
            FabFireError::InvalidCredentials(credentials) => {
                write!(f, "InvalidCredentials: {}", credentials)
            }
            FabFireError::Session(err) => write!(f, "Session: {}", err),
        }
    }
}

impl MechanismError for FabFireError {
    fn kind(&self) -> MechanismErrorKind {
        match self {
            FabFireError::ParseError => MechanismErrorKind::Parse,
            FabFireError::SerializationError => MechanismErrorKind::Protocol,
            FabFireError::DeserializationError(_) => MechanismErrorKind::Parse,
            FabFireError::CardError(_) => MechanismErrorKind::Protocol,
            FabFireError::InvalidMagic(_) => MechanismErrorKind::Protocol,
            FabFireError::InvalidToken(_) => MechanismErrorKind::Protocol,
            FabFireError::InvalidURN(_) => MechanismErrorKind::Protocol,
            FabFireError::InvalidCredentials(_) => MechanismErrorKind::Protocol,
            FabFireError::Session(_) => MechanismErrorKind::Protocol,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CardInfo {
    #[serde(rename = "UID", with = "hex")]
    uid: [u8; 7],
    key_old: Option<Box<[u8]>>,
    key_new: Option<Box<[u8]>>,
}

struct KeyInfo {
    key_id: u8,
    key: Box<[u8]>,
}

struct AuthInfo {
    rnd_a: Vec<u8>,
    rnd_b: Vec<u8>,
    iv: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "Cmd")]
enum CardCommand {
    message {
        #[serde(rename = "MssgID", skip_serializing_if = "Option::is_none")]
        msg_id: Option<u32>,
        #[serde(rename = "ClrTxt", skip_serializing_if = "Option::is_none")]
        clr_txt: Option<String>,
        #[serde(rename = "AddnTxt", skip_serializing_if = "Option::is_none")]
        addn_txt: Option<String>,
    },
    sendPICC {
        #[serde(
            deserialize_with = "hex::deserialize",
            serialize_with = "hex::serialize_upper"
        )]
        data: Vec<u8>,
    },
    readPICC {
        #[serde(
            deserialize_with = "hex::deserialize",
            serialize_with = "hex::serialize_upper"
        )]
        data: Vec<u8>,
    },
    haltPICC,
    Key {
        data: String,
    },
    ConfirmUser,
}

enum Step {
    New,
    SelectApp,
    VerifyMagic,
    GetURN,
    GetToken,
    Authenticate1,
    Authenticate2,
}

pub struct FabFire {
    step: Step,
    card_info: Option<CardInfo>,
    key_info: Option<KeyInfo>,
    auth_info: Option<AuthInfo>,
    app_id: u32,
    local_urn: String,
    desfire: Desfire,
}

const MAGIC: &'static str = "FABACCESS\0DESFIRE\01.0\0";

impl FabFire {
    pub fn new_server(_sasl: &SASL) -> Result<Box<dyn Authentication>, SASLError> {
        Ok(Box::new(Self {
            step: Step::New,
            card_info: None,
            key_info: None,
            auth_info: None,
            app_id: 0x464142,
            local_urn: "urn:fabaccess:lab:innovisionlab".to_string(),
            desfire: Desfire {
                card: None,
                session_key: None,
                cbc_iv: None,
            },
        }))
    }
}

impl Authentication for FabFire {
    fn step(
        &mut self,
        session: &mut SessionData,
        input: Option<&[u8]>,
        writer: &mut dyn Write,
    ) -> StepResult {
        match self.step {
            Step::New => {
                tracing::trace!("Step: New");
                //receive card info (especially card UID) from reader
                return match input {
                    None => Err(SessionError::InputDataRequired),
                    Some(cardinfo) => {
                        self.card_info = match serde_json::from_slice(cardinfo) {
                            Ok(card_info) => Some(card_info),
                            Err(e) => {
                                tracing::error!("Deserializing card_info failed: {:?}", e);
                                return Err(FabFireError::DeserializationError(e).into());
                            }
                        };
                        //select application
                        let buf = match self.desfire.select_application_cmd(self.app_id) {
                            Ok(buf) => match Vec::<u8>::try_from(buf) {
                                Ok(data) => data,
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to convert APDUCommand to Vec<u8>: {:?}",
                                        e
                                    );
                                    return Err(FabFireError::SerializationError.into());
                                }
                            },
                            Err(e) => {
                                tracing::error!("Failed to generate APDUCommand: {:?}", e);
                                return Err(FabFireError::SerializationError.into());
                            }
                        };
                        let cmd = CardCommand::sendPICC { data: buf };
                        return match serde_json::to_vec(&cmd) {
                            Ok(send_buf) => {
                                self.step = Step::SelectApp;
                                writer
                                    .write_all(&send_buf)
                                    .map_err(|e| SessionError::Io { source: e })?;
                                Ok(rsasl::session::Step::NeedsMore(Some(send_buf.len())))
                            }
                            Err(e) => {
                                tracing::error!("Failed to serialize APDUCommand: {:?}", e);
                                Err(FabFireError::SerializationError.into())
                            }
                        };
                    }
                };
            }
            Step::SelectApp => {
                tracing::trace!("Step: SelectApp");
                // check that we successfully selected the application
                let response: CardCommand = match input {
                    None => {
                        return Err(SessionError::InputDataRequired);
                    }
                    Some(buf) => match serde_json::from_slice(buf)
                        .map_err(|e| FabFireError::DeserializationError(e))
                    {
                        Ok(response) => response,
                        Err(e) => {
                            tracing::error!("Deserializing data from card failed: {:?}", e);
                            return Err(e.into());
                        }
                    },
                };

                let apdu_response = match response {
                    CardCommand::readPICC { data } => APDUResponse::new(&*data),
                    _ => {
                        tracing::error!("Unexpected response: {:?}", response);
                        return Err(FabFireError::ParseError.into());
                    }
                };

                apdu_response
                    .check()
                    .map_err(|e| FabFireError::CardError(e))?;

                // request the contents of the file containing the magic string
                const MAGIC_FILE_ID: u8 = 0x01;

                let buf = match self
                    .desfire
                    .read_data_chunk_cmd(MAGIC_FILE_ID, 0, MAGIC.len())
                {
                    Ok(buf) => match Vec::<u8>::try_from(buf) {
                        Ok(data) => data,
                        Err(e) => {
                            tracing::error!("Failed to convert APDUCommand to Vec<u8>: {:?}", e);
                            return Err(FabFireError::SerializationError.into());
                        }
                    },
                    Err(e) => {
                        tracing::error!("Failed to generate APDUCommand: {:?}", e);
                        return Err(FabFireError::SerializationError.into());
                    }
                };
                let cmd = CardCommand::sendPICC { data: buf };
                return match serde_json::to_vec(&cmd) {
                    Ok(send_buf) => {
                        self.step = Step::VerifyMagic;
                        writer
                            .write_all(&send_buf)
                            .map_err(|e| SessionError::Io { source: e })?;
                        Ok(rsasl::session::Step::NeedsMore(Some(send_buf.len())))
                    }
                    Err(e) => {
                        tracing::error!("Failed to serialize APDUCommand: {:?}", e);
                        Err(FabFireError::SerializationError.into())
                    }
                };
            }
            Step::VerifyMagic => {
                tracing::trace!("Step: VerifyMagic");
                // verify the magic string to determine that we have a valid fabfire card
                let response: CardCommand = match input {
                    None => {
                        return Err(SessionError::InputDataRequired);
                    }
                    Some(buf) => match serde_json::from_slice(buf)
                        .map_err(|e| FabFireError::DeserializationError(e))
                    {
                        Ok(response) => response,
                        Err(e) => {
                            tracing::error!("Deserializing data from card failed: {:?}", e);
                            return Err(e.into());
                        }
                    },
                };

                let apdu_response = match response {
                    CardCommand::readPICC { data } => APDUResponse::new(&*data),
                    _ => {
                        tracing::error!("Unexpected response: {:?}", response);
                        return Err(FabFireError::ParseError.into());
                    }
                };

                match apdu_response.check() {
                    Ok(_) => {
                        match apdu_response.body {
                            Some(data) => {
                                if std::str::from_utf8(data.as_slice()) != Ok(MAGIC) {
                                    tracing::error!("Invalid magic string");
                                    return Err(FabFireError::ParseError.into());
                                }
                            }
                            None => {
                                tracing::error!("No data returned from card");
                                return Err(FabFireError::ParseError.into());
                            }
                        };
                    }
                    Err(e) => {
                        tracing::error!("Got invalid APDUResponse: {:?}", e);
                        return Err(FabFireError::ParseError.into());
                    }
                }

                // request the contents of the file containing the URN
                const URN_FILE_ID: u8 = 0x02;

                let buf = match self.desfire.read_data_chunk_cmd(
                    URN_FILE_ID,
                    0,
                    self.local_urn.as_bytes().len(),
                ) {
                    // TODO: support urn longer than 47 Bytes
                    Ok(buf) => match Vec::<u8>::try_from(buf) {
                        Ok(data) => data,
                        Err(e) => {
                            tracing::error!("Failed to convert APDUCommand to Vec<u8>: {:?}", e);
                            return Err(FabFireError::SerializationError.into());
                        }
                    },
                    Err(e) => {
                        tracing::error!("Failed to generate APDUCommand: {:?}", e);
                        return Err(FabFireError::SerializationError.into());
                    }
                };
                let cmd = CardCommand::sendPICC { data: buf };
                return match serde_json::to_vec(&cmd) {
                    Ok(send_buf) => {
                        self.step = Step::GetURN;
                        writer
                            .write_all(&send_buf)
                            .map_err(|e| SessionError::Io { source: e })?;
                        Ok(rsasl::session::Step::NeedsMore(Some(send_buf.len())))
                    }
                    Err(e) => {
                        tracing::error!("Failed to serialize APDUCommand: {:?}", e);
                        Err(FabFireError::SerializationError.into())
                    }
                };
            }
            Step::GetURN => {
                tracing::trace!("Step: GetURN");
                // parse the urn and match it to our local urn
                let response: CardCommand = match input {
                    None => {
                        return Err(SessionError::InputDataRequired);
                    }
                    Some(buf) => match serde_json::from_slice(buf)
                        .map_err(|e| FabFireError::DeserializationError(e))
                    {
                        Ok(response) => response,
                        Err(e) => {
                            tracing::error!("Deserializing data from card failed: {:?}", e);
                            return Err(e.into());
                        }
                    },
                };

                let apdu_response = match response {
                    CardCommand::readPICC { data } => APDUResponse::new(&*data),
                    _ => {
                        tracing::error!("Unexpected response: {:?}", response);
                        return Err(FabFireError::ParseError.into());
                    }
                };

                match apdu_response.check() {
                    Ok(_) => {
                        match apdu_response.body {
                            Some(data) => {
                                let received_urn = String::from_utf8(data).unwrap();
                                if received_urn != self.local_urn {
                                    tracing::error!(
                                        "URN mismatch: {:?} != {:?}",
                                        received_urn,
                                        self.local_urn
                                    );
                                    return Err(FabFireError::ParseError.into());
                                }
                            }
                            None => {
                                tracing::error!("No data returned from card");
                                return Err(FabFireError::ParseError.into());
                            }
                        };
                    }
                    Err(e) => {
                        tracing::error!("Got invalid APDUResponse: {:?}", e);
                        return Err(FabFireError::ParseError.into());
                    }
                }
                // request the contents of the file containing the URN
                const TOKEN_FILE_ID: u8 = 0x03;

                let buf = match self.desfire.read_data_chunk_cmd(
                    TOKEN_FILE_ID,
                    0,
                    MAX_BYTES_PER_TRANSACTION,
                ) {
                    // TODO: support data longer than 47 Bytes
                    Ok(buf) => match Vec::<u8>::try_from(buf) {
                        Ok(data) => data,
                        Err(e) => {
                            tracing::error!("Failed to convert APDUCommand to Vec<u8>: {:?}", e);
                            return Err(FabFireError::SerializationError.into());
                        }
                    },
                    Err(e) => {
                        tracing::error!("Failed to generate APDUCommand: {:?}", e);
                        return Err(FabFireError::SerializationError.into());
                    }
                };
                let cmd = CardCommand::sendPICC { data: buf };
                return match serde_json::to_vec(&cmd) {
                    Ok(send_buf) => {
                        self.step = Step::GetToken;
                        writer
                            .write_all(&send_buf)
                            .map_err(|e| SessionError::Io { source: e })?;
                        Ok(rsasl::session::Step::NeedsMore(Some(send_buf.len())))
                    }
                    Err(e) => {
                        tracing::error!("Failed to serialize APDUCommand: {:?}", e);
                        Err(FabFireError::SerializationError.into())
                    }
                };
            }
            Step::GetToken => {
                // println!("Step: GetToken");
                // parse the token and select the appropriate user
                let response: CardCommand = match input {
                    None => {
                        return Err(SessionError::InputDataRequired);
                    }
                    Some(buf) => match serde_json::from_slice(buf)
                        .map_err(|e| FabFireError::DeserializationError(e))
                    {
                        Ok(response) => response,
                        Err(e) => {
                            tracing::error!("Deserializing data from card failed: {:?}", e);
                            return Err(e.into());
                        }
                    },
                };

                let apdu_response = match response {
                    CardCommand::readPICC { data } => APDUResponse::new(&*data),
                    _ => {
                        tracing::error!("Unexpected response: {:?}", response);
                        return Err(FabFireError::ParseError.into());
                    }
                };

                match apdu_response.check() {
                    Ok(_) => {
                        match apdu_response.body {
                            Some(data) => {
                                let token = String::from_utf8(data).unwrap();
                                session.set_property::<AuthId>(Arc::new(
                                    token.trim_matches(char::from(0)).to_string(),
                                ));
                                let key = match session.get_property_or_callback::<FabFireCardKey>()
                                {
                                    Ok(Some(key)) => Box::from(key.as_slice()),
                                    Ok(None) => {
                                        tracing::error!("No keys on file for token");
                                        return Err(FabFireError::InvalidCredentials(
                                            "No keys on file for token".to_string(),
                                        )
                                        .into());
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to get key: {:?}", e);
                                        return Err(FabFireError::Session(e).into());
                                    }
                                };
                                self.key_info = Some(KeyInfo { key_id: 0x01, key });
                            }
                            None => {
                                tracing::error!("No data in response");
                                return Err(FabFireError::ParseError.into());
                            }
                        };
                    }
                    Err(e) => {
                        tracing::error!("Failed to check response: {:?}", e);
                        return Err(FabFireError::ParseError.into());
                    }
                }

                let buf = match self
                    .desfire
                    .authenticate_iso_aes_challenge_cmd(self.key_info.as_ref().unwrap().key_id)
                {
                    Ok(buf) => match Vec::<u8>::try_from(buf) {
                        Ok(data) => data,
                        Err(e) => {
                            tracing::error!("Failed to convert to Vec<u8>: {:?}", e);
                            return Err(FabFireError::SerializationError.into());
                        }
                    },
                    Err(e) => {
                        tracing::error!("Failed to create authenticate command: {:?}", e);
                        return Err(FabFireError::SerializationError.into());
                    }
                };
                let cmd = CardCommand::sendPICC { data: buf };
                return match serde_json::to_vec(&cmd) {
                    Ok(send_buf) => {
                        self.step = Step::Authenticate1;
                        writer
                            .write_all(&send_buf)
                            .map_err(|e| SessionError::Io { source: e })?;
                        Ok(rsasl::session::Step::NeedsMore(Some(send_buf.len())))
                    }
                    Err(e) => {
                        tracing::error!("Failed to serialize command: {:?}", e);
                        Err(FabFireError::SerializationError.into())
                    }
                };
            }
            Step::Authenticate1 => {
                tracing::trace!("Step: Authenticate1");
                let response: CardCommand = match input {
                    None => {
                        return Err(SessionError::InputDataRequired);
                    }
                    Some(buf) => match serde_json::from_slice(buf)
                        .map_err(|e| FabFireError::DeserializationError(e))
                    {
                        Ok(response) => response,
                        Err(e) => {
                            tracing::error!("Failed to deserialize response: {:?}", e);
                            return Err(e.into());
                        }
                    },
                };

                let apdu_response = match response {
                    CardCommand::readPICC { data } => APDUResponse::new(&*data),
                    _ => {
                        tracing::error!("Unexpected response: {:?}", response);
                        return Err(FabFireError::ParseError.into());
                    }
                };

                match apdu_response.check() {
                    Ok(_) => {
                        match apdu_response.body {
                            Some(data) => {
                                let rnd_b_enc = data.as_slice();

                                //FIXME: This is ugly, we should find a better way to make the function testable
                                //TODO: Check if we need a CSPRNG here
                                let rnd_a: [u8; 16] = rand::random();

                                let (cmd_challenge_response, rnd_b, iv) = self
                                    .desfire
                                    .authenticate_iso_aes_response_cmd(
                                        rnd_b_enc,
                                        &*(self.key_info.as_ref().unwrap().key),
                                        &rnd_a,
                                    )
                                    .unwrap();
                                self.auth_info = Some(AuthInfo {
                                    rnd_a: Vec::<u8>::from(rnd_a),
                                    rnd_b,
                                    iv,
                                });
                                let buf = match Vec::<u8>::try_from(cmd_challenge_response) {
                                    Ok(data) => data,
                                    Err(e) => {
                                        tracing::error!("Failed to convert to Vec<u8>: {:?}", e);
                                        return Err(FabFireError::SerializationError.into());
                                    }
                                };
                                let cmd = CardCommand::sendPICC { data: buf };
                                return match serde_json::to_vec(&cmd) {
                                    Ok(send_buf) => {
                                        self.step = Step::Authenticate2;
                                        writer
                                            .write_all(&send_buf)
                                            .map_err(|e| SessionError::Io { source: e })?;
                                        Ok(rsasl::session::Step::NeedsMore(Some(send_buf.len())))
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to serialize command: {:?}", e);
                                        Err(FabFireError::SerializationError.into())
                                    }
                                };
                            }
                            None => {
                                tracing::error!("Got invalid response: {:?}", apdu_response);
                                return Err(FabFireError::ParseError.into());
                            }
                        };
                    }
                    Err(e) => {
                        tracing::error!("Failed to check response: {:?}", e);
                        return Err(FabFireError::ParseError.into());
                    }
                }
            }
            Step::Authenticate2 => {
                // println!("Step: Authenticate2");
                let response: CardCommand = match input {
                    None => {
                        return Err(SessionError::InputDataRequired);
                    }
                    Some(buf) => match serde_json::from_slice(buf)
                        .map_err(|e| FabFireError::DeserializationError(e))
                    {
                        Ok(response) => response,
                        Err(e) => {
                            tracing::error!("Failed to deserialize response: {:?}", e);
                            return Err(e.into());
                        }
                    },
                };

                let apdu_response = match response {
                    CardCommand::readPICC { data } => APDUResponse::new(&*data),
                    _ => {
                        tracing::error!("Got invalid response: {:?}", response);
                        return Err(FabFireError::ParseError.into());
                    }
                };

                match apdu_response.check() {
                    Ok(_) => {
                        match apdu_response.body {
                            Some(data) => match self.auth_info.as_ref() {
                                None => {
                                    return Err(FabFireError::ParseError.into());
                                }
                                Some(auth_info) => {
                                    if self
                                        .desfire
                                        .authenticate_iso_aes_verify(
                                            data.as_slice(),
                                            auth_info.rnd_a.as_slice(),
                                            auth_info.rnd_b.as_slice(),
                                            &*(self.key_info.as_ref().unwrap().key),
                                            auth_info.iv.as_slice(),
                                        )
                                        .is_ok()
                                    {
                                        let cmd = CardCommand::message {
                                            msg_id: Some(4),
                                            clr_txt: None,
                                            addn_txt: Some("".to_string()),
                                        };
                                        return match serde_json::to_vec(&cmd) {
                                            Ok(send_buf) => {
                                                self.step = Step::Authenticate1;
                                                writer
                                                    .write_all(&send_buf)
                                                    .map_err(|e| SessionError::Io { source: e })?;
                                                return Ok(rsasl::session::Step::Done(Some(
                                                    send_buf.len(),
                                                )));
                                            }
                                            Err(e) => {
                                                tracing::error!(
                                                    "Failed to serialize command: {:?}",
                                                    e
                                                );
                                                Err(FabFireError::SerializationError.into())
                                            }
                                        };
                                    }
                                }
                            },
                            None => {
                                tracing::error!("got empty response");
                                return Err(FabFireError::ParseError.into());
                            }
                        };
                    }
                    Err(_e) => {
                        tracing::error!("Got invalid response: {:?}", apdu_response);
                        return Err(
                            FabFireError::InvalidCredentials(format!("{}", apdu_response)).into(),
                        );
                    }
                }
            }
        }

        return Ok(rsasl::session::Step::Done(None));
    }
}
