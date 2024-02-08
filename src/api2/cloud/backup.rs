use anyhow::Error;
use serde_json::Value;
use proxmox_schema::api;
use proxmox_router::{list_subdirs_api_method, Router, SubdirMap};

// pub const ROUTER: Router = Router::new()
//     .get(&API_METHOD_CLOUD_HELLO_BACKUP);

const SUBDIRS: SubdirMap = &[
    ("status", &Router::new().get(&API_METHOD_CLOUD_HELLO_BACKUP)),
];

const ITEM_ROUTER: Router = Router::new()
    .get(&list_subdirs_api_method!(SUBDIRS))
    .subdirs(SUBDIRS);

pub const ROUTER: Router = Router::new()
    .get(&API_METHOD_CLOUD_HELLO_BACKUP)
    .match_all("name", &ITEM_ROUTER);

#[api(
    input: {
        properties: {},
    },
    returns: {
        description: "Cloud hello backup.",
        type: String,
    },
)]
/// Cloud Hello
pub fn cloud_hello_backup(_param: Value) -> Result<String, Error> {
    let prm = _param.to_string();
    Ok(format!("api2/json/cloud/backup cloud-hello-world and value is: {}", prm))
}