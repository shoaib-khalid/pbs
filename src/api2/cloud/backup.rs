use anyhow::{bail, format_err, Error};
use serde_json::Value;

use proxmox_router::{Permission, Router, RpcEnvironment, RpcEnvironmentType};
use proxmox_schema::api;

const CLOUD_BACKUP_JOB_ROUTER: Router = Router::new().post(&API_METHOD_RUN_CLOUD_BACKUP_JOB);

pub const ROUTER: Router = Router::new()
    .get(&API_METHOD_LIST_CLOUD_BACKUP_JOBS)
    .post(&API_METHOD_BACKUP)
    .match_all("id", &CLOUD_BACKUP_JOB_ROUTER);

#[api(
        returns: {
            description: "List configured cloud backup jobs and their status",
            type: String,
        },
        access: {
            description: "List configured cloud jobs filtered by Cloud.Audit privileges - to be implemented for cloud",
            permission: &Permission::Anybody,
        },
    )]
/// List all cloud backup jobs
pub fn list_cloud_backup_jobs() -> String {
    let returnString: String = "This is the list of cloud backup jobs";
    returnString
}

// Returns a string containing the list of cloud backup jobs when called.
#[test]
fn returns_string_containing_list_of_cloud_backup_jobs() {
    let result = list_cloud_backup_jobs();
    assert!(result.contains("This is the list of cloud backup jobs"));
}

#[api(
    returns: {
        description: "Run all cloud backup job",
        type: String,
    },
    access: {
        description: "List configured cloud jobs filtered by Cloud.Audit privileges - to be implemented for cloud",
        permission: &Permission::Anybody,
    },
)]
pub fn run_cloud_backup_job() -> String {
    let cloud_backup_job_String: String = "Reply from Cloud run backup job";
    cloud_backup_job_String
}
#[api(
    returns: {
        description: "Cloud backup",
        type: String,
    },
    access: {
        description: "Cloud Backup privilege(s) - to be implemented",
        permission: &Permission::Anybody,
    },
)]
/// List all cloud backup jobs
pub fn backup() -> String {
    let returnString: String = "This is cloud backup method";
    returnString
}
