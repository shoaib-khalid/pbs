use anyhow::{format_err, Error};

use crate::{
    server::WorkerTask,
    api2::types::*,
    server::jobstate::Job,
    config::verify::VerificationJobConfig,
    backup::{
        DataStore,
        verify_filter,
        verify_all_backups,
    },
    task_log,
};

/// Runs a verification job.
pub fn do_verification_job(
    mut job: Job,
    verification_job: VerificationJobConfig,
    auth_id: &Authid,
    schedule: Option<String>,
) -> Result<String, Error> {

    let datastore = DataStore::lookup_datastore(&verification_job.store)?;

    let outdated_after = verification_job.outdated_after;
    let ignore_verified_snapshots = verification_job.ignore_verified.unwrap_or(true);

    let (email, notify) = crate::server::lookup_datastore_notify_settings(&verification_job.store);

    let job_id = format!("{}:{}",
                         &verification_job.store,
                         job.jobname());
    let worker_type = job.jobtype().to_string();
    let upid_str = WorkerTask::new_thread(
        &worker_type,
        Some(job_id.clone()),
        auth_id.clone(),
        false,
        move |worker| {
            job.start(&worker.upid().to_string())?;

            task_log!(worker,"Starting datastore verify job '{}'", job_id);
            if let Some(event_str) = schedule {
                task_log!(worker,"task triggered by schedule '{}'", event_str);
            }

            let verify_worker = crate::backup::VerifyWorker::new(worker.clone(), datastore);
            let result = verify_all_backups(
                &verify_worker,
                worker.upid(),
                None,
                Some(&move |manifest| {
                    verify_filter(ignore_verified_snapshots, outdated_after, manifest)
                }),
            );
            let job_result = match result {
                Ok(ref failed_dirs) if failed_dirs.is_empty() => Ok(()),
                Ok(ref failed_dirs) => {
                    worker.log("Failed to verify the following snapshots/groups:");
                    for dir in failed_dirs {
                        worker.log(format!("\t{}", dir));
                    }

                    Err(format_err!("verification failed - please check the log for details"))
                },
                Err(_) => Err(format_err!("verification failed - job aborted")),
            };

            let status = worker.create_state(&job_result);

            if let Err(err) = job.finish(status) {
                eprintln!(
                    "could not finish job state for {}: {}",
                    job.jobtype().to_string(),
                    err
                );
            }

            if let Some(email) = email {
                if let Err(err) = crate::server::send_verify_status(&email, notify, verification_job, &result) {
                    eprintln!("send verify notification failed: {}", err);
                }
            }

            job_result
        },
    )?;
    Ok(upid_str)
}
