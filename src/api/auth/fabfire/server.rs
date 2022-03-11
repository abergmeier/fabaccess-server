use std::fmt::{Debug, Display, Formatter};
use std::io::Write;
use rsasl::error::{MechanismError, MechanismErrorKind, SASLError, SessionError};
use rsasl::mechanism::Authentication;
use rsasl::SASL;
use rsasl::session::{SessionData, StepResult};
use serde::{Deserialize, Serialize};
use desfire::desfire::Desfire;
use desfire::iso7816_4::apducommand::APDUCommand;
use desfire::iso7816_4::apduresponse::APDUResponse;
use desfire::error::{Error as DesfireError, Error};
use std::convert::TryFrom;
use std::ops::Deref;

enum FabFireError {
    ParseError,
    SerializationError,
    CardError(DesfireError),
}

impl Debug for FabFireError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FabFireError::ParseError => write!(f, "ParseError"),
            FabFireError::SerializationError => write!(f, "SerializationError"),
            FabFireError::CardError(err) => write!(f, "CardError: {}", err),
        }
    }
}

impl Display for FabFireError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FabFireError::ParseError => write!(f, "ParseError"),
            FabFireError::SerializationError => write!(f, "SerializationError"),
            FabFireError::CardError(err) => write!(f, "CardError: {}", err),
        }
    }
}

impl MechanismError for FabFireError {
    fn kind(&self) -> MechanismErrorKind {
        match self {
            FabFireError::ParseError => MechanismErrorKind::Parse,
            FabFireError::SerializationError => MechanismErrorKind::Protocol,
            FabFireError::CardError(_) => MechanismErrorKind::Protocol,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct CardInfo {
    card_uid: [u8; 7],
    key_old: Option<Box<[u8]>>,
    key_new: Option<Box<[u8]>>
}

struct KeyInfo {
    key_id: u8,
    key: Box<[u8]>
}

struct AuthInfo {
    rnd_a: Vec<u8>,
    rnd_b: Vec<u8>,
    iv: Vec<u8>
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "Cmd")]
enum CardCommand {
    message {
        msg_id: Option<u32>,
        clr_txt: Option<String>,
        addn_txt: Option<String>,
    },
    sendPICC {
        data: String
    },
    haltPICC,
    Key {
        data: String
    },
    ConfirmUser
}

enum Step {
    New,
    SelectApp,
    VerifyMagic,
    GetURN,
    GetToken,
    Authenticate1,
    Authenticate2,
    Authenticate3,
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
        Ok(Box::new(Self { step: Step::New, card_info: None, key_info: None, auth_info: None, app_id: 1, local_urn: "urn:fabaccess:lab:innovisionlab".to_string(), desfire: Desfire { card: None, session_key: None, cbc_iv: None } }))
    }
}

impl Authentication for FabFire {
    fn step(&mut self, session: &mut SessionData, input: Option<&[u8]>, writer: &mut dyn Write) -> StepResult {
        match self.step {
            Step::New => {
                //receive card info (especially card UID) from reader
                return match input {
                    None => { Err(SessionError::InputDataRequired) },
                    Some(cardinfo) => {
                        self.card_info = match serde_json::from_slice(cardinfo) {
                            Ok(card_info) => Some(card_info),
                            Err(_) => {
                                return Err(FabFireError::ParseError.into())
                            }
                        };
                        self.step = Step::SelectApp;
                        Ok(rsasl::session::Step::NeedsMore(None))
                    }
                }
            }
            Step::SelectApp => {
                //select application
                let buf = match self.desfire.select_application_cmd(self.app_id) {
                    Ok(buf) => match Vec::<u8>::try_from(buf) {
                        Ok(data) => data,
                        Err(_) => {
                            return Err(FabFireError::SerializationError.into())
                        }
                    },
                    Err(_) => {
                        return Err(FabFireError::SerializationError.into())
                    }
                };
                let cmd = CardCommand::sendPICC { data: hex::encode_upper(buf) };
                return match serde_json::to_writer(writer, &cmd) {
                    Ok(_) => {
                        self.step = Step::VerifyMagic;
                        Ok(rsasl::session::Step::NeedsMore(None))
                    }
                    Err(_) => {
                        Err(FabFireError::SerializationError.into())
                    }
                }
            }
            Step::VerifyMagic => {
                // check that we successfully selected the application
                let response = match input {
                    None => {return Err(SessionError::InputDataRequired)},
                    Some(buf) => APDUResponse::new(buf)
                };
                response.check().map_err(|e| FabFireError::CardError(e))?;

                // request the contents of the file containing the magic string
                const MAGIC_FILE_ID: u8 = 0x01;

                let buf = match self.desfire.read_data_chunk_cmd(MAGIC_FILE_ID, 0, MAGIC.len()) {
                    Ok(buf) => match Vec::<u8>::try_from(buf) {
                        Ok(data) => data,
                        Err(_) => {
                            return Err(FabFireError::SerializationError.into())
                        }
                    },
                    Err(_) => {
                        return Err(FabFireError::SerializationError.into())
                    }
                };
                let cmd = CardCommand::sendPICC { data: hex::encode_upper(buf) };
                return match serde_json::to_writer(writer, &cmd) {
                    Ok(_) => {
                        self.step = Step::GetURN;
                        Ok(rsasl::session::Step::NeedsMore(None))
                    }
                    Err(_) => {
                        Err(FabFireError::SerializationError.into())
                    }
                }
            }
            Step::GetURN => {
                // verify the magic string to determine that we have a valid fabfire card
                let response = match input {
                    None => {return Err(SessionError::InputDataRequired)},
                    Some(buf) => APDUResponse::new(buf)
                };
                match response.check() {
                    Ok(_) => {
                        match response.body {
                            Some(data) => {
                                if std::str::from_utf8(data.as_slice()) != Ok(MAGIC) {
                                    return Err(FabFireError::ParseError.into());
                                }
                            }
                            None => {
                                return Err(FabFireError::ParseError.into())
                            }
                        };
                    }
                    Err(_) => {
                        return Err(FabFireError::ParseError.into());
                    }
                }


                // request the contents of the file containing the URN
                const URN_FILE_ID: u8 = 0x02;

                let buf = match self.desfire.read_data_chunk_cmd(URN_FILE_ID, 0, self.local_urn.as_bytes().len()) { // TODO: support urn longer than 47 Bytes
                    Ok(buf) => match Vec::<u8>::try_from(buf) {
                        Ok(data) => data,
                        Err(_) => {
                            return Err(FabFireError::SerializationError.into())
                        }
                    },
                    Err(_) => {
                        return Err(FabFireError::SerializationError.into())
                    }
                };
                let cmd = CardCommand::sendPICC { data: hex::encode_upper(buf) };
                return match serde_json::to_writer(writer, &cmd) {
                    Ok(_) => {
                        self.step = Step::GetToken;
                        Ok(rsasl::session::Step::NeedsMore(None))
                    }
                    Err(_) => {
                        Err(FabFireError::SerializationError.into())
                    }
                }
            }
            Step::GetToken => {
                // parse the urn and match it to our local urn
                let response = match input {
                    None => {return Err(SessionError::InputDataRequired)},
                    Some(buf) => APDUResponse::new(buf)
                };
                match response.check() {
                    Ok(_) => {
                        match response.body {
                            Some(data) => {
                                if String::from_utf8(data).unwrap() != self.local_urn {
                                    return Err(FabFireError::ParseError.into());
                                }
                            }
                            None => {
                                return Err(FabFireError::ParseError.into())
                            }
                        };
                    }
                    Err(_) => {
                        return Err(FabFireError::ParseError.into());
                    }
                }
                // request the contents of the file containing the URN
                const TOKEN_FILE_ID: u8 = 0x03;

                let buf = match self.desfire.read_data_chunk_cmd(TOKEN_FILE_ID, 0, 47) { // TODO: support data longer than 47 Bytes
                    Ok(buf) => match Vec::<u8>::try_from(buf) {
                        Ok(data) => data,
                        Err(_) => {
                            return Err(FabFireError::SerializationError.into())
                        }
                    },
                    Err(_) => {
                        return Err(FabFireError::SerializationError.into())
                    }
                };
                let cmd = CardCommand::sendPICC { data: hex::encode_upper(buf) };
                return match serde_json::to_writer(writer, &cmd) {
                    Ok(_) => {
                        self.step = Step::Authenticate1;
                        Ok(rsasl::session::Step::NeedsMore(None))
                    }
                    Err(_) => {
                        Err(FabFireError::SerializationError.into())
                    }
                }
            }
            Step::Authenticate1 => {
                // parse the token and select the appropriate user
                let response = match input {
                    None => {return Err(SessionError::InputDataRequired)},
                    Some(buf) => APDUResponse::new(buf)
                };
                match response.check() {
                    Ok(_) => {
                        match response.body {
                            Some(data) => {
                                if String::from_utf8(data).unwrap() != "LoremIpsum" { // FIXME: match against user db
                                    return Err(FabFireError::ParseError.into());
                                }
                            }
                            None => {
                                return Err(FabFireError::ParseError.into())
                            }
                        };
                    }
                    Err(_) => {
                        return Err(FabFireError::ParseError.into());
                    }
                }

                let buf = match self.desfire.authenticate_iso_aes_challenge_cmd(self.key_info.as_ref().unwrap().key_id) {
                    Ok(buf) => match Vec::<u8>::try_from(buf) {
                        Ok(data) => data,
                        Err(_) => {
                            return Err(FabFireError::SerializationError.into())
                        }
                    },
                    Err(_) => {
                        return Err(FabFireError::SerializationError.into())
                    }
                };
                let cmd = CardCommand::sendPICC { data: hex::encode_upper(buf) };
                return match serde_json::to_writer(writer, &cmd) {
                    Ok(_) => {
                        self.step = Step::Authenticate2;
                        Ok(rsasl::session::Step::NeedsMore(None))
                    }
                    Err(_) => {
                        Err(FabFireError::SerializationError.into())
                    }
                }

            }
            Step::Authenticate2 => {
                let response = match input {
                    None => {return Err(SessionError::InputDataRequired)},
                    Some(buf) => APDUResponse::new(buf)
                };
                match response.check() {
                    Ok(_) => {
                        match response.body {
                            Some(data) => {
                                let rnd_b_enc = data.as_slice();

                                //FIXME: This is ugly, we should find a better way to make the function testable
                                //TODO: Check if we need a CSPRNG here
                                let rnd_a: [u8; 16] = rand::random();
                                println!("RND_A: {:x?}", rnd_a);

                                let (cmd_challenge_response, rnd_b, iv) = self.desfire.authenticate_iso_aes_response_cmd(rnd_b_enc, &*(self.key_info.as_ref().unwrap().key), &rnd_a).unwrap();
                                self.auth_info = Some(AuthInfo{rnd_a: Vec::<u8>::from(rnd_a), rnd_b, iv});
                                let buf = match Vec::<u8>::try_from(cmd_challenge_response) {
                                        Ok(data) => data,
                                        Err(_) => {
                                            return Err(FabFireError::SerializationError.into())
                                        }
                                };
                                let cmd = CardCommand::sendPICC { data: hex::encode_upper(buf) };
                                return match serde_json::to_writer(writer, &cmd) {
                                    Ok(_) => {
                                        self.step = Step::Authenticate3;
                                        Ok(rsasl::session::Step::NeedsMore(None))
                                    }
                                    Err(_) => {
                                        Err(FabFireError::SerializationError.into())
                                    }
                                }
                            }
                            None => {
                                return Err(FabFireError::ParseError.into())
                            }
                        };
                    }
                    Err(_) => {
                        return Err(FabFireError::ParseError.into());
                    }
                }
            }
            Step::Authenticate3 => {
                let response = match input {
                    None => {return Err(SessionError::InputDataRequired)},
                    Some(buf) => APDUResponse::new(buf)
                };
                match response.check() {
                    Ok(_) => {
                        match response.body {
                            Some(data) => {
                                match self.auth_info.as_ref() {
                                    None => {return Err(FabFireError::ParseError.into())}
                                    Some(auth_info) => {
                                        if self.desfire.authenticate_iso_aes_verify(
                                            data.as_slice(),
                                            auth_info.rnd_a.as_slice(),
                                            auth_info.rnd_b.as_slice(), &*(self.key_info.as_ref().unwrap().key),
                                            auth_info.iv.as_slice()).is_ok() {
                                            // TODO: Do stuff with the info that we are authenticated
                                            return Ok(rsasl::session::Step::Done(None));
                                        }
                                    }
                                }
                            }
                            None => {
                                return Err(FabFireError::ParseError.into())
                            }
                        };
                    }
                    Err(_) => {
                        return Err(FabFireError::ParseError.into());
                    }
                }
            }
        }

        return Ok(rsasl::session::Step::Done(None));
    }

}