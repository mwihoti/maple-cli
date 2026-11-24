//! Stage consented TUI diagnostics for feedback uploads and local ZIP saves.

use crate::app_event::FeedbackDestination;
use crate::app_event::FeedbackReceipt;
use crate::app_server_session::fs::AppServerFileSystem;
use crate::app_server_session::is_method_unsupported;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use color_eyre::eyre::eyre;
use maplecode_app_server_client::AppServerPath;
use maplecode_app_server_client::AppServerRequestHandle;
use maplecode_app_server_client::TypedRequestError;
use maplecode_app_server_protocol::ClientRequest;
use maplecode_app_server_protocol::FeedbackSaveParams;
use maplecode_app_server_protocol::FeedbackSaveResponse;
use maplecode_app_server_protocol::FeedbackUploadParams;
use maplecode_app_server_protocol::FeedbackUploadResponse;
use maplecode_app_server_protocol::RequestId;
use maplecode_feedback::MaplecodeFeedback;
use maplecode_utils_path_uri::LegacyAppPathString;
use std::path::PathBuf;
use uuid::Uuid;

// Base64 adds only a third to this size, keeping fs/writeFile below the 16 MiB frame limit.
const MAX_CLIENT_LOG_SIZE: usize = 1024 * 1024;
const FEEDBACK_DIR: &str = "feedback";

pub(super) async fn deliver_feedback(
    request_handle: AppServerRequestHandle,
    maplecode_home: Option<AppServerPath>,
    mut params: FeedbackUploadParams,
    feedback: MaplecodeFeedback,
    destination: FeedbackDestination,
) -> Result<FeedbackReceipt> {
    let fs = AppServerFileSystem { request_handle };
    let staging_dir = maplecode_home
        .as_ref()
        .filter(|_| params.include_logs)
        .map(|home| {
            home.join("tmp")
                .join(format!("tui-feedback-{}", Uuid::new_v4()))
        });
    if params.include_logs {
        let staged = async {
            let directory = staging_dir.as_ref().ok_or_else(|| {
                eyre!("App server did not report $MAPLECODE_HOME; cannot stage feedback logs")
            })?;
            // These RPCs use the app-server's local filesystem, where both feedback methods
            // read extraLogFiles, independently of the thread's executor environment.
            fs.fs_create_directory_all_path(directory).await?;
            let path = directory.join("client-logs.txt");
            let mut logs = feedback
                .snapshot(/*session_id*/ None)
                .log_attachment(/*logs_override*/ None)
                .buffer;
            logs.drain(..logs.len().saturating_sub(MAX_CLIENT_LOG_SIZE));
            fs.fs_write_file_path(&path, logs).await?;
            Ok::<_, color_eyre::Report>(path)
        }
        .await;
        match staged {
            Ok(path) => params
                .extra_log_files
                .get_or_insert_with(Vec::new)
                .push(PathBuf::from(path.as_str())),
            Err(err) => {
                tracing::warn!("failed to stage TUI feedback logs: {err}");
                params.reason.get_or_insert_with(String::new).push_str(
                    "\n\nTUI client logs were omitted because they could not be staged on the app-server.",
                );
            }
        }
    }
    let result = match destination {
        FeedbackDestination::OpenAi => fs
            .request_handle
            .request_typed::<FeedbackUploadResponse>(ClientRequest::FeedbackUpload {
                request_id: RequestId::String(format!("feedback-upload-{}", Uuid::new_v4())),
                params,
            })
            .await
            .map(|response| FeedbackReceipt::Uploaded {
                thread_id: response.thread_id,
            })
            .wrap_err("Failed to upload feedback"),
        FeedbackDestination::LocalZip => save_feedback_zip(&fs, maplecode_home.as_ref(), params)
            .await
            .wrap_err("Failed to save feedback ZIP"),
    };

    if let Some(directory) = staging_dir
        && let Err(err) = fs.fs_remove_path(&directory).await
    {
        tracing::warn!("failed to remove staged feedback logs: {err}");
    }
    result
}

async fn save_feedback_zip(
    fs: &AppServerFileSystem,
    maplecode_home: Option<&AppServerPath>,
    params: FeedbackUploadParams,
) -> Result<FeedbackReceipt> {
    let home = maplecode_home.ok_or_else(|| {
        eyre!(crate::history_cell::branded(
            "The app server did not report its Maple Code home directory",
            "The app server did not report its Maple Code home directory"
        ))
    })?;
    let directory = home.join(FEEDBACK_DIR);
    fs.fs_create_directory_all_path(&directory)
        .await
        .wrap_err_with(|| format!("Could not create the feedback directory at {directory}"))?;
    let id = Uuid::new_v4();
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%SZ");
    let suffix = id.as_fields().0;
    let path = directory.join(format!(
        "maplecode-diagnostics-{timestamp}-{suffix:08x}.zip"
    ));
    let response: FeedbackSaveResponse = fs
        .request_handle
        .request_typed(ClientRequest::FeedbackSave {
            request_id: RequestId::String(format!("feedback-save-{id}")),
            params: FeedbackSaveParams {
                feedback: params,
                save_path: LegacyAppPathString::from_string(path.as_str()),
            },
        })
        .await
        .map_err(|err| match &err {
            TypedRequestError::Server { source, .. }
                if is_method_unsupported(source, "feedback/save") =>
            {
                eyre!(crate::history_cell::branded(
                    "Update the Maple Code server to save a feedback ZIP",
                    "Update the Maple Code server to save a feedback ZIP"
                ))
            }
            _ => color_eyre::Report::from(err),
        })?;
    Ok(FeedbackReceipt::Saved {
        path: response.saved_path.to_string(),
    })
}
