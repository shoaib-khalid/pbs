use std::sync::{Arc, Mutex};

use anyhow::{bail, format_err, Error};
use serde_json::Value;

use proxmox_lang::try_block;
use proxmox_router::{Permission, Router, RpcEnvironment, RpcEnvironmentType};
use proxmox_schema::api;
use proxmox_sys::{task_log, task_warn, WorkerTaskContext};

use pbs_api_types::{
    print_ns_and_snapshot, print_store_and_ns, Authid, MediaPoolConfig, Operation,
    TapeBackupJobConfig, TapeBackupJobSetup, TapeBackupJobStatus, Userid, JOB_ID_SCHEMA,
    PRIV_DATASTORE_READ, PRIV_TAPE_AUDIT, PRIV_TAPE_WRITE, UPID_SCHEMA,
};

use pbs_config::CachedUserInfo;
use pbs_datastore::backup_info::{BackupDir, BackupInfo};
use pbs_datastore::{DataStore, StoreProgress};
use proxmox_rest_server::WorkerTask;

use crate::{
    server::{
        jobstate::{compute_schedule_status, Job, JobState},
        lookup_user_email, TapeBackupJobSummary,
    },
    tape::{
        changer::update_changer_online_status,
        drive::{lock_tape_device, media_changer, set_tape_device_state, TapeLockError},
        Inventory, MediaPool, PoolWriter, TAPE_STATUS_DIR,
    },
};

// const TAPE_BACKUP_JOB_ROUTER: Router = Router::new().post(&API_METHOD_RUN_TAPE_BACKUP_JOB);

const KALSYM_ROUTER: Router = Router::new().get(&API_METHOD_RUN_KALSYM);

pub const ROUTER: Router = Router::new()
    .get(&API_METHOD_RUN_KALSYM);
    // .post(&API_METHOD_BACKUP)
    // .match_all("id", &KALSYM_ROUTER);

    #[api(
        input: {
            properties: {},
        },
        returns: {
            description: "kalsym Cloud hello world.",
            type: String,
        },
    )]
    /// List kalsym jobs
    pub fn run_kalsym(_param: Value) -> Result<String, Error> {
        Ok("hello kalsym world".to_string())
    }

