use desfire::desfire::desfire::MAX_BYTES_PER_TRANSACTION;
use desfire::desfire::Desfire;
use desfire::error::Error as DesfireError;
use desfire::iso7816_4::apduresponse::APDUResponse;
use rsasl::callback::SessionData;
use rsasl::mechanism::{
    Authentication, Demand, DemandReply, MechanismData, MechanismError, MechanismErrorKind,
    Provider, State, ThisProvider,
};
use rsasl::prelude::{MessageSent, SASLConfig, SASLError, SessionError};
use rsasl::property::AuthId;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt::{Debug, Display, Formatter};
use std::io::Write;
use std::sync::Arc;

use crate::authentication::fabfire::FabFireCardKey;
use crate::CONFIG;

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

impl std::error::Error for FabFireError {}

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
    authid: String,
    key_id: u8,
    key: Box<[u8]>,
}

struct AuthInfo {
    rnd_a: Vec<u8>,
    rnd_b: Vec<u8>,
    iv: Vec<u8>,
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
    pub fn new_server(_sasl: &SASLConfig) -> Result<Box<dyn Authentication>, SASLError> {
        let space = if let Some(space) = CONFIG.get().map(|c| c.spacename.as_str()) {
            space
        } else {
            tracing::error!("No space configured");
            "generic"
        };

        Ok(Box::new(Self {
            step: Step::New,
            card_info: None,
            key_info: None,
            auth_info: None,
            app_id: 0x464142,
            local_urn: format!("urn:fabaccess:lab:{space}"),
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
        session: &mut MechanismData<'_>,
        input: Option<&[u8]>,
        writer: &mut dyn Write,
    ) -> Result<State, SessionError> {
        match self.step {
            Step::New => {
                tracing::trace!("Step: New");
                //receive card info (especially card UID) from reader
                return match input {
                    None => Err(SessionError::InputDataRequired),
                    Some(_) => {
                        //select application
                        return match self.desfire.select_application_cmd(self.app_id) {
                            Ok(buf) => match Vec::<u8>::try_from(buf) {
                                Ok(data) => {
                                    self.step = Step::SelectApp;
                                    writer
                                        .write_all(&data)
                                        .map_err(|e| SessionError::Io { source: e })?;
                                    Ok(State::Running)
                                }
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
                    }
                };
            }
            Step::SelectApp => {
                tracing::trace!("Step: SelectApp");
                // check that we successfully selected the application

                let apdu_response = match input {
                    Some(data) => APDUResponse::new(data),
                    None => return Err(SessionError::InputDataRequired),
                };

                apdu_response
                    .check()
                    .map_err(|e| FabFireError::CardError(e))?;

                // request the contents of the file containing the magic string
                const MAGIC_FILE_ID: u8 = 0x01;

                return match self
                    .desfire
                    .read_data_chunk_cmd(MAGIC_FILE_ID, 0, MAGIC.len())
                {
                    Ok(buf) => match Vec::<u8>::try_from(buf) {
                        Ok(data) => {
                            self.step = Step::VerifyMagic;
                            writer
                                .write_all(&data)
                                .map_err(|e| SessionError::Io { source: e })?;
                            Ok(State::Running)
                        }
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
            }
            Step::VerifyMagic => {
                tracing::trace!("Step: VerifyMagic");
                // verify the magic string to determine that we have a valid fabfire card
                let apdu_response = match input {
                    Some(data) => APDUResponse::new(data),
                    None => return Err(SessionError::InputDataRequired),
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

                return match self.desfire.read_data_chunk_cmd(
                    URN_FILE_ID,
                    0,
                    self.local_urn.as_bytes().len(),
                ) {
                    // TODO: support urn longer than 47 Bytes
                    Ok(buf) => match Vec::<u8>::try_from(buf) {
                        Ok(data) => {
                            self.step = Step::GetURN;
                            writer
                                .write_all(&data)
                                .map_err(|e| SessionError::Io { source: e })?;
                            Ok(State::Running)
                        }
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
            }
            Step::GetURN => {
                tracing::trace!("Step: GetURN");
                // parse the urn and match it to our local urn
                let apdu_response = match input {
                    Some(data) => APDUResponse::new(data),
                    None => return Err(SessionError::InputDataRequired),
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

                return match self.desfire.read_data_chunk_cmd(
                    TOKEN_FILE_ID,
                    0,
                    MAX_BYTES_PER_TRANSACTION,
                ) {
                    // TODO: support data longer than 47 Bytes
                    Ok(buf) => match Vec::<u8>::try_from(buf) {
                        Ok(data) => {
                            self.step = Step::GetToken;
                            writer
                                .write_all(&data)
                                .map_err(|e| SessionError::Io { source: e })?;
                            Ok(State::Running)
                        }
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
            }
            Step::GetToken => {
                // println!("Step: GetToken");
                // parse the token and select the appropriate user
                let apdu_response = match input {
                    Some(data) => APDUResponse::new(data),
                    None => return Err(SessionError::InputDataRequired),
                };

                match apdu_response.check() {
                    Ok(_) => {
                        match apdu_response.body {
                            Some(data) => {
                                let authid = String::from_utf8(data)
                                    .unwrap()
                                    .trim_matches(char::from(0))
                                    .to_string();
                                let prov = ThisProvider::<AuthId>::with(&authid);
                                let key = session
                                    .need_with::<FabFireCardKey, _, _>(&prov, |key| {
                                        Ok(Box::from(key.as_slice()))
                                    })?;
                                self.key_info = Some(KeyInfo {
                                    authid,
                                    key_id: 0x01,
                                    key,
                                });
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

                return match self
                    .desfire
                    .authenticate_iso_aes_challenge_cmd(self.key_info.as_ref().unwrap().key_id)
                {
                    Ok(buf) => match Vec::<u8>::try_from(buf) {
                        Ok(data) => {
                            self.step = Step::Authenticate1;
                            writer
                                .write_all(&data)
                                .map_err(|e| SessionError::Io { source: e })?;
                            Ok(State::Running)
                        }
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
            }
            Step::Authenticate1 => {
                tracing::trace!("Step: Authenticate1");
                let apdu_response = match input {
                    Some(data) => APDUResponse::new(data),
                    None => return Err(SessionError::InputDataRequired),
                };

                return match apdu_response.check() {
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
                                match Vec::<u8>::try_from(cmd_challenge_response) {
                                    Ok(data) => {
                                        self.step = Step::Authenticate2;
                                        writer
                                            .write_all(&data)
                                            .map_err(|e| SessionError::Io { source: e })?;
                                        Ok(State::Running)
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to convert to Vec<u8>: {:?}", e);
                                        return Err(FabFireError::SerializationError.into());
                                    }
                                }
                            }
                            None => {
                                tracing::error!("Got invalid response: {:?}", apdu_response);
                                Err(FabFireError::ParseError.into())
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to check response: {:?}", e);
                        Err(FabFireError::ParseError.into())
                    }
                };
            }
            Step::Authenticate2 => {
                // println!("Step: Authenticate2");
                let apdu_response = match input {
                    Some(data) => APDUResponse::new(data),
                    None => return Err(SessionError::InputDataRequired),
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
                                        struct Prov<'a> {
                                            authid: &'a str,
                                        }
                                        impl<'a> Provider<'a> for Prov<'a> {
                                            fn provide(
                                                &self,
                                                req: &mut Demand<'a>,
                                            ) -> DemandReply<()>
                                            {
                                                req.provide_ref::<AuthId>(self.authid)?.done()
                                            }
                                        }
                                        let prov = Prov {
                                            authid: &self.key_info.as_ref().unwrap().authid,
                                        };
                                        session.validate(&prov)?;
                                        return Ok(State::Finished(MessageSent::Yes));
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

        return Ok(State::Finished(MessageSent::No));
    }
}
