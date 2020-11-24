use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::db::access::{
    Permission,
    PermissionBuf,
    PermRule,
    RoleIdentifier,
    Role,
};

lazy_static! {
    static ref AUTH_PERM: &'static Permission = Permission::new("bffh.auth");
}

// 
// lazy_static! {
//     pub static ref AUTH_ROLE: RoleIdentifier = {
//         RoleIdentifier::Local {
//             name: "mayauth".to_string(),
//             source: "builtin".to_string(),
//         }
//     };
// }
// 
// lazy_static! {
//     pub static ref DEFAULT_ROLEIDS: [RoleIdentifier; 1] = {
//         [ AUTH_ROLE.clone(), ]
//     };
// 
//     pub static ref DEFAULT_ROLES: HashMap<RoleIdentifier, Role> = {
//         let mut m = HashMap::new();
//         m.insert(AUTH_ROLE.clone(),
//             Role {
//                 parents: vec![],
//                 permissions: vec![
//                     PermRule::Base(PermissionBuf::from_perm(AUTH_PERM)),
//                 ]
//             }
//         );
//         m
//     };
// }
