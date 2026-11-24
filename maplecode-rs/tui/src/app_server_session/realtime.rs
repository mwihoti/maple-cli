//! App-server signaling for TUI-owned realtime WebRTC sessions.

use super::AppServerSession;
use maplecode_app_server_protocol::ClientRequest;
use maplecode_app_server_protocol::ThreadRealtimeAppendSpeechParams;
use maplecode_app_server_protocol::ThreadRealtimeAppendSpeechResponse;
use maplecode_app_server_protocol::ThreadRealtimeStartParams;
use maplecode_app_server_protocol::ThreadRealtimeStartResponse;
use maplecode_app_server_protocol::ThreadRealtimeStartTransport;
use maplecode_app_server_protocol::ThreadRealtimeStopParams;
use maplecode_app_server_protocol::ThreadRealtimeStopResponse;
use maplecode_protocol::ThreadId;
use maplecode_protocol::protocol::RealtimeConversationVersion;
use maplecode_protocol::protocol::RealtimeOutputModality;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;

impl AppServerSession {
    pub(crate) async fn thread_realtime_start(
        &mut self,
        thread_id: ThreadId,
        call_id: String,
    ) -> Result<()> {
        let request_id = self.next_request_id();
        let _: ThreadRealtimeStartResponse = self
            .request_handle
            .request_typed(ClientRequest::ThreadRealtimeStart {
                request_id,
                params: ThreadRealtimeStartParams {
                    thread_id: thread_id.to_string(),
                    client_managed_handoffs: Some(true),
                    delegation_ack_filler: None,
                    flush_transcript_tail_on_session_end: None,
                    maplecode_responses_as_items: None,
                    maplecode_response_item_prefix: None,
                    maplecode_response_handoff_mode: None,
                    backend_reasoning_status: false,
                    maplecode_response_handoff_channel_prefixes: None,
                    model: None,
                    output_modality: RealtimeOutputModality::Audio,
                    include_startup_context: Some(false),
                    initial_items: None,
                    realtime_start_instructions: None,
                    realtime_end_instructions: None,
                    prompt: None,
                    realtime_session_id: Some(thread_id.to_string()),
                    voice_session_id: None,
                    transport: Some(ThreadRealtimeStartTransport::ExistingCall {
                        // The TUI owns the peer connection and has already negotiated this call.
                        call_id,
                    }),
                    version: Some(RealtimeConversationVersion::V3),
                    voice: None,
                },
            })
            .await
            .wrap_err("thread/realtime/start failed in TUI")?;
        Ok(())
    }

    pub(crate) async fn thread_realtime_stop(&mut self, thread_id: ThreadId) -> Result<()> {
        let request_id = self.next_request_id();
        let _: ThreadRealtimeStopResponse = self
            .request_handle
            .request_typed(ClientRequest::ThreadRealtimeStop {
                request_id,
                params: ThreadRealtimeStopParams {
                    thread_id: thread_id.to_string(),
                },
            })
            .await
            .wrap_err("thread/realtime/stop failed in TUI")?;
        Ok(())
    }

    pub(crate) async fn thread_realtime_append_speech(
        &mut self,
        thread_id: ThreadId,
        text: String,
    ) -> Result<()> {
        let request_id = self.next_request_id();
        let _: ThreadRealtimeAppendSpeechResponse = self
            .request_handle
            .request_typed(ClientRequest::ThreadRealtimeAppendSpeech {
                request_id,
                params: ThreadRealtimeAppendSpeechParams {
                    thread_id: thread_id.to_string(),
                    text,
                },
            })
            .await
            .wrap_err("thread/realtime/appendSpeech failed in TUI")?;
        Ok(())
    }
}
