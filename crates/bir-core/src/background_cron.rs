use crate::db::Database;
use crate::forms::form_2551q::FilingStatus;
use crate::profile::TaxpayerProfile;
use chrono::{Duration, Utc, Datelike};
use std::sync::{Arc, Mutex};
use std::str::FromStr;
use tracing::{error, info, warn};

pub async fn start_cron_jobs(db: Arc<Mutex<Database>>) {
    info!("Background cron engine started");

    loop {
        // Run every 1 minute
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        
        info!("Running background cron jobs...");
        
        // Ensure we don't hold the DB lock across async network calls.
        // First, fetch profiles that have background cron enabled.
        let profiles = {
            let db_guard = match db.lock() {
                Ok(guard) => guard,
                Err(e) => {
                    error!("Failed to lock database for cron: {}", e);
                    continue;
                }
            };
            match db_guard.list_profiles() {
                Ok(p) => p,
                Err(e) => {
                    error!("Failed to fetch profiles in cron: {}", e);
                    continue;
                }
            }
        };

        let test_enabled = profiles.iter().any(|p| p.test_notification_enabled);
        if test_enabled {
            let _ = notify_rust::Notification::new()
                .summary("BIR Vault Daemon")
                .body("Hello! The background cron is active.")
                .show();
        }

        let cron_profiles = profiles.into_iter().filter(|p| p.background_cron_enabled).collect::<Vec<_>>();

        for profile in cron_profiles {
            // Task A: Form Submission Retries
            process_submission_queue(&profile, db.clone()).await;
        }

        // Task C: Generic Job Queue (Custom Cron & One-off commands)
        process_generic_jobs(db.clone()).await;
    }
}

async fn process_submission_queue(profile: &TaxpayerProfile, db: Arc<Mutex<Database>>) {
    let current_year = Utc::now().naive_utc().date().year() as u16;
    
    // We only retry current year forms to avoid excessive queries, 
    // but we can query `list_draft_summaries` for the profile.
    let summaries = {
        let db_guard = match db.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        db_guard.list_draft_summaries(&profile.tin.full(), current_year).unwrap_or_default()
    };

    for summary in summaries.into_iter().filter(|s| s.status == FilingStatus::Queued) {
        let mut draft = {
            let db_guard = match db.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            if summary.form_code == "2551Q" {
                match db_guard.get_2551q_draft(&profile.tin.full(), summary.taxable_year, summary.quarter.unwrap_or(0)) {
                    Ok(Some(d)) => d,
                    _ => continue,
                }
            } else {
                continue; // other forms not supported yet
            }
        };

        // Check if we should retry now
        if let Some(next_retry) = &draft.next_retry_at {
            if let Ok(next_time) = chrono::DateTime::parse_from_rfc3339(next_retry) {
                if Utc::now() < next_time.with_timezone(&Utc) {
                    continue; // not time yet
                }
            }
        }

        info!("Cron: Attempting to submit queued form {} for {}", draft.period_code(), profile.tin.full());
        
        let form_type = "2551Qv2018"; // hardcoded for 2551Q for now
        let filename = draft.default_submission_filename();
        let xml_payload = draft.to_bir_xml_payload();
        let passphrase = "T0081gP45sy0rd-To+R3m3m63r!@4/<>";
        
        let encrypted = match crate::crypto::compress_and_encrypt(xml_payload.as_bytes(), passphrase) {
            Ok(enc) => enc,
            Err(e) => {
                fail_draft(&mut draft, db.clone(), e.to_string());
                continue;
            }
        };

        match crate::transport::submit_iaf(form_type, &filename, &encrypted).await {
            Ok(_) => {
                info!("Cron: Successfully submitted queued form {}", filename);
                draft.status = FilingStatus::Submitted;
                draft.submitted_at = Some(Utc::now().to_rfc3339());
                draft.submission_filename = Some(filename);
                draft.submission_attempts = 0;
                draft.next_retry_at = None;
                draft.last_error = None;
                
                if let Ok(db_guard) = db.lock() {
                    let _ = db_guard.save_2551q_draft(&draft);
                    
                    if profile.is_email_tracking_active() {
                        let email = profile.imap_email.clone().unwrap_or_else(|| profile.email.clone());
                        let job_name = format!("Poll Receipts: {}", email);
                        
                        let jobs = db_guard.list_jobs().unwrap_or_default();
                        let mut exists = false;
                        for job in jobs.iter() {
                            if job.name == job_name && job.status != "Archived" && job.status != "Done" {
                                exists = true;
                                break;
                            }
                        }
                        
                        if !exists {
                            let new_job = crate::db::Job {
                                id: None,
                                name: job_name,
                                job_type: "System".to_string(),
                                cron_expr: Some("0 * * * * *".to_string()),
                                command: Some(format!("bir_poll_email {}", email)),
                                status: "Queued".to_string(),
                                retries: 0,
                                last_run_at: None,
                                next_run_at: None,
                                created_at: Utc::now().to_rfc3339(),
                            };
                            let _ = db_guard.save_job(new_job);
                        }
                    }
                }
            }
            Err(e) => {
                fail_draft(&mut draft, db.clone(), e.to_string());
            }
        }
    }
}

fn fail_draft(draft: &mut crate::forms::form_2551q::Form2551QDraft, db: Arc<Mutex<Database>>, error_msg: String) {
    warn!("Cron: Submission failed for {}: {}", draft.period_code(), error_msg);
    draft.submission_attempts += 1;
    draft.last_error = Some(error_msg);
    
    // Exponential backoff up to 5 attempts
    if draft.submission_attempts >= 5 {
        warn!("Cron: Max attempts reached for {}. Giving up.", draft.period_code());
        draft.status = FilingStatus::Draft; // Revert to draft
        draft.next_retry_at = None;
    } else {
        // 1min, 2min, 4min, 8min
        let delay_mins = 2i64.pow(draft.submission_attempts - 1);
        let next_time = Utc::now() + Duration::minutes(delay_mins);
        draft.next_retry_at = Some(next_time.to_rfc3339());
        info!("Cron: Next retry scheduled in {} mins", delay_mins);
    }
    
    if let Ok(db_guard) = db.lock() {
        let _ = db_guard.save_2551q_draft(draft);
    }
}



async fn process_generic_jobs(db: Arc<Mutex<Database>>) {
    let jobs = {
        let db_guard = match db.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        db_guard.list_jobs().unwrap_or_default()
    };

    let now = Utc::now();

    for mut job in jobs {
        // Self-heal legacy bad cron strings
        if let Some(ref mut expr) = job.cron_expr {
            if expr.trim() == "* * * * *" {
                *expr = "0 * * * * *".to_string();
                if job.status == "Failed" {
                    job.status = "Queued".to_string();
                }
                if let Ok(db_guard) = db.lock() {
                    let _ = db_guard.save_job(job.clone());
                }
            }
        }

        let should_run = if let Some(ref next_run_str) = job.next_run_at {
            if let Ok(next_time) = chrono::DateTime::parse_from_rfc3339(next_run_str) {
                now >= next_time.with_timezone(&Utc)
            } else {
                false
            }
        } else {
            // No next run defined.
            if job.status == "Queued" {
                if let Some(ref expr) = job.cron_expr {
                    if !expr.trim().is_empty() {
                        // Initialize next run time
                        if let Ok(schedule) = cron::Schedule::from_str(expr) {
                            if let Some(next_run) = schedule.upcoming(Utc).next() {
                                job.next_run_at = Some(next_run.to_rfc3339());
                                if let Ok(db_guard) = db.lock() {
                                    let _ = db_guard.save_job(job.clone());
                                }
                            }
                        }
                        false // Wait for scheduled time
                    } else {
                        true // One-off job, run now
                    }
                } else {
                    true // One-off job, run now
                }
            } else {
                false
            }
        };

        if !should_run {
            continue;
        }

        // Set status to Running
        job.status = "Running".to_string();
        if let Ok(db_guard) = db.lock() {
            let _ = db_guard.save_job(job.clone());
        }

        info!("Cron: Executing job '{}'", job.name);
        let mut success = true;

        if let Some(ref cmd) = job.command {
            if !cmd.trim().is_empty() {
                if cmd.starts_with("bir_poll_email ") {
                    let email = cmd.trim_start_matches("bir_poll_email ").trim();
                    let (poll_success, still_pending) = crate::email::fetch_and_process_emails_for_address(email, db.clone());
                    if poll_success {
                        info!("Cron: Email polling job '{}' completed successfully.", job.name);
                        success = true;
                        if !still_pending {
                            job.status = "Archived".to_string(); // Completed processing for this email
                        }
                    } else {
                        warn!("Cron: Email polling job '{}' failed.", job.name);
                        success = false;
                    }
                } else {
                    match tokio::process::Command::new("sh").arg("-c").arg(cmd).output().await {
                        Ok(output) => {
                            if output.status.success() {
                                info!("Cron: Job '{}' completed successfully.", job.name);
                            } else {
                                let err_output = String::from_utf8_lossy(&output.stderr);
                                warn!("Cron: Job '{}' failed with code: {:?} stderr: {}", job.name, output.status, err_output);
                                success = false;
                            }
                        }
                        Err(e) => {
                            warn!("Cron: Job '{}' failed to start: {}", job.name, e);
                            success = false;
                        }
                    }
                }
            }
        }

        job.last_run_at = Some(now.to_rfc3339());

        if let Some(ref cron_expr) = job.cron_expr {
            if !cron_expr.trim().is_empty() {
                if let Ok(schedule) = cron::Schedule::from_str(cron_expr) {
                    if let Some(next_run) = schedule.upcoming(Utc).next() {
                        job.next_run_at = Some(next_run.to_rfc3339());
                        job.status = "Queued".to_string();
                        job.retries = if success { 0 } else { job.retries + 1 };
                    } else {
                        job.status = "Done".to_string(); // cron will never run again
                    }
                } else {
                    warn!("Cron: Invalid cron expression for job '{}'", job.name);
                    job.status = "Failed".to_string();
                }
                if let Ok(db_guard) = db.lock() {
                    let _ = db_guard.save_job(job.clone());
                }
            } else {
                // Treated as one-off if empty string
                if success {
                    if let Ok(db_guard) = db.lock() {
                        let _ = db_guard.delete_job(job.id.unwrap());
                    }
                } else {
                    job.retries += 1;
                    job.status = "Failed".to_string();
                    if let Ok(db_guard) = db.lock() {
                        let _ = db_guard.save_job(job.clone());
                    }
                }
            }
        } else {
            // One-off job
            if success {
                if let Ok(db_guard) = db.lock() {
                    let _ = db_guard.delete_job(job.id.unwrap());
                }
            } else {
                job.retries += 1;
                job.status = "Failed".to_string();
                if let Ok(db_guard) = db.lock() {
                    let _ = db_guard.save_job(job.clone());
                }
            }
        }
    }
}
