
//! FabAccess generated API bindings
//!
//! This crate contains slightly nicer and better documented bindings for the FabAccess API.

#[allow(dead_code)]
pub mod schema;

/// Authentication subsystem
pub mod auth {
    /// Session authentication
    ///
    /// Authentication uses a SASL exchange. To bootstrap a connection you will need to call
    /// `step` until you get a successful result
    pub mod authentication {
        pub use crate::schema::auth_capnp::authentication::*;
    }

    pub mod response {
        pub use crate::schema::auth_capnp::response::*;
    }
}

pub mod resource {
    pub use crate::schema::resource_capnp::*;
}

pub mod resources {
    pub use crate::schema::resources_capnp::*;
}

pub mod role {
    pub use crate::schema::role_capnp::*;
}

pub mod user {
    pub use crate::schema::user_capnp::user::*;
}

pub mod users {
    pub use crate::schema::users_capnp::*;
}

pub mod utils {
    pub mod uuid {
        pub use crate::schema::utils_capnp::u_u_i_d::*;
    }

    /// Localization String
    ///
    /// This is a specialized string that allows to access the string contents in different
    /// languages
    pub mod l10n_string {
        pub use crate::schema::utils_capnp::l10_n_string::*;
    }
}

pub mod bootstrap {
    pub use crate::schema::main_capnp::bootstrap::*;
}

pub mod session {
    pub use crate::schema::main_capnp::session::*;
}