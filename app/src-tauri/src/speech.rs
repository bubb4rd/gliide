//! Spoken practice (DESIGN.md, Job → Interview → Practice): your answer said out loud, heard by macOS's own speech
//! recognition (Speech framework) with the audio never leaving this Mac (`requiresOnDeviceRecognition`; a language
//! without an on-device model is unavailable rather than sent to Apple). What's heard so far goes to the UI as
//! `speech` events ([`Hearing`]); stopping returns the answer with how long you spoke and its words ([`Heard`]), which
//! the practice turn keeps. No model call: the answer then goes to Practice as a typed one does.
//!
//! One session at a time, on a thread of its own that owns the audio engine, the request and the task (none of them
//! `Send`): the commands talk to it over channels. macOS asks for the microphone and for speech recognition the first
//! time only, from Speak; both need their usage descriptions in the app's Info.plist (`app/src-tauri/Info.plist`), or
//! asking aborts the process, so a binary without them reports unavailable.

use std::sync::Arc;

use tauri::AppHandle;

use crate::dto::{Hearing, PrivacyPane, SpeechState, SpeechStatus};
use crate::error::{AppError, Result};

/// Where what's heard goes while the microphone is open (the app: the `speech` event).
pub type Hear = Arc<dyn Fn(Hearing) + Send + Sync>;

/// Words in `text`, as you'd count them.
fn words(text: &str) -> u32 {
    u32::try_from(text.split_whitespace().count()).unwrap_or(u32::MAX)
}

/// What's been heard: macOS's recognizer sometimes starts its transcription over after a pause, so a result that
/// reads as a fresh start keeps the one before it.
#[derive(Debug, Default)]
struct Transcript {
    /// Transcriptions it started over from, in order.
    kept: Vec<String>,
    /// The latest transcription.
    current: String,
}

impl Transcript {
    fn heard(&mut self, text: &str) {
        let text = text.trim();
        if restarted(&self.current, text) {
            self.kept.push(std::mem::take(&mut self.current));
        }
        text.clone_into(&mut self.current);
    }

    fn text(&self) -> String {
        let mut all = self.kept.join(" ");
        if !self.current.is_empty() {
            if !all.is_empty() {
                all.push(' ');
            }
            all.push_str(&self.current);
        }
        all
    }
}

/// `now` starts over rather than refining `before`: a refinement reworks the words but never loses half of them.
fn restarted(before: &str, now: &str) -> bool {
    now.len() * 2 < before.len()
}

fn denied(pane: PrivacyPane) -> SpeechStatus {
    let (what, name) = match pane {
        PrivacyPane::Microphone => ("The microphone", "Microphone"),
        PrivacyPane::Speech => ("Speech recognition", "Speech Recognition"),
    };
    SpeechStatus {
        state: SpeechState::Denied,
        reason: Some(format!(
            "{what} is off for gliide. Turn it on in System Settings → Privacy & Security → {name}."
        )),
        pane: Some(pane),
    }
}

fn unavailable(reason: &str) -> SpeechStatus {
    SpeechStatus {
        state: SpeechState::Unavailable,
        reason: Some(reason.to_owned()),
        pane: None,
    }
}

/// System Settings → Privacy & Security, on `pane`.
pub fn open_settings(handle: &AppHandle, pane: PrivacyPane) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;

    let url = match pane {
        PrivacyPane::Microphone => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
        }
        PrivacyPane::Speech => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_SpeechRecognition"
        }
    };
    handle
        .opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::other(format!("Couldn’t open System Settings: {e}")))
}

#[cfg(target_os = "macos")]
pub use mac::{cancel, start, status, stop};

#[cfg(not(target_os = "macos"))]
pub use other::{cancel, start, status, stop};

#[cfg(not(target_os = "macos"))]
mod other {
    use super::*;
    use crate::dto::Heard;

    const NOT_HERE: &str = "Spoken practice needs macOS for now.";

    pub fn status() -> SpeechStatus {
        unavailable(NOT_HERE)
    }

    pub async fn start(_: super::Hear, _: Vec<String>) -> Result<()> {
        Err(AppError::other(NOT_HERE))
    }

    pub async fn stop() -> Result<Heard> {
        Err(AppError::other(NOT_HERE))
    }

    pub fn cancel() {}
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ptr::NonNull;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::sync::{Arc, Mutex, MutexGuard};
    use std::time::{Duration, Instant};

    use block2::RcBlock;
    use objc2::rc::{Retained, autoreleasepool};
    use objc2::runtime::Bool;
    use objc2::{AllocAnyThread, available};
    use objc2_avf_audio::{
        AVAudioApplication, AVAudioApplicationRecordPermission, AVAudioEngine, AVAudioPCMBuffer,
        AVAudioTime,
    };
    use objc2_foundation::{NSArray, NSBundle, NSError, NSOperationQueue, NSString};
    use objc2_speech::{
        SFSpeechAudioBufferRecognitionRequest, SFSpeechRecognitionResult, SFSpeechRecognitionTask,
        SFSpeechRecognitionTaskHint, SFSpeechRecognizer, SFSpeechRecognizerAuthorizationStatus,
    };
    use tokio::sync::oneshot;

    use super::{Hear, Transcript, denied, unavailable, words};
    use crate::dto::{Heard, Hearing, PrivacyPane, SpeechState, SpeechStatus, Spoken};
    use crate::error::{AppError, Result};

    /// The longest answer heard in one go; the microphone closes after it, keeping what was heard.
    const MAX: Duration = Duration::from_secs(10 * 60);
    /// How long the recognizer may take to finish once the microphone closes.
    const FINISH: Duration = Duration::from_secs(5);
    /// Words it may hear that a recognizer wouldn't guess.
    const MAX_HINTS: usize = 100;
    const NO_ON_DEVICE: &str = "macOS can’t hear your language on this Mac without sending it to Apple. Turn on \
                                Dictation in System Settings → Keyboard to add it.";

    /// The session running: its end, and the answer it hears.
    struct Session {
        end: mpsc::Sender<End>,
        done: oneshot::Receiver<Result<Heard>>,
    }

    enum End {
        Stop,
        Cancel,
    }

    static SESSION: Mutex<Option<Session>> = Mutex::new(None);

    fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
        m.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Asking macOS aborts the process without these in the main bundle's Info.plist.
    fn described() -> bool {
        let bundle = NSBundle::mainBundle();
        [
            "NSMicrophoneUsageDescription",
            "NSSpeechRecognitionUsageDescription",
        ]
        .iter()
        .all(|k| {
            bundle
                .objectForInfoDictionaryKey(&NSString::from_str(k))
                .is_some()
        })
    }

    fn speech_permission() -> SFSpeechRecognizerAuthorizationStatus {
        // SAFETY: a class method with no arguments.
        unsafe { SFSpeechRecognizer::authorizationStatus() }
    }

    /// The microphone's permission; before macOS 14 it can't be read, and macOS asks when it first opens.
    fn mic_permission() -> AVAudioApplicationRecordPermission {
        if available!(macos = 14.0) {
            // SAFETY: the shared instance, then a property read.
            unsafe { AVAudioApplication::sharedInstance().recordPermission() }
        } else {
            AVAudioApplicationRecordPermission::Granted
        }
    }

    /// A recognizer for your language that hears on this Mac.
    fn recognizer() -> Option<Retained<SFSpeechRecognizer>> {
        // SAFETY: `init` on a fresh allocation; nil when no recognizer serves your language.
        let r = unsafe { SFSpeechRecognizer::init(SFSpeechRecognizer::alloc()) }?;
        // SAFETY: a property read.
        unsafe { r.supportsOnDeviceRecognition() }.then_some(r)
    }

    pub fn status() -> SpeechStatus {
        autoreleasepool(|_| {
            if !described() {
                return unavailable("This build of gliide can’t use the microphone.");
            }
            let speech = speech_permission();
            if matches!(
                speech,
                SFSpeechRecognizerAuthorizationStatus::Denied
                    | SFSpeechRecognizerAuthorizationStatus::Restricted
            ) {
                return denied(PrivacyPane::Speech);
            }
            let mic = mic_permission();
            if mic == AVAudioApplicationRecordPermission::Denied {
                return denied(PrivacyPane::Microphone);
            }
            if recognizer().is_none() {
                return unavailable(NO_ON_DEVICE);
            }
            let asked = speech != SFSpeechRecognizerAuthorizationStatus::NotDetermined
                && mic != AVAudioApplicationRecordPermission::Undetermined;
            SpeechStatus {
                state: if asked {
                    SpeechState::Ready
                } else {
                    SpeechState::Ask
                },
                reason: None,
                pane: None,
            }
        })
    }

    /// A one-shot sender a completion block can own: the block is `Fn`, so the sender is taken once.
    fn once<T>(tx: oneshot::Sender<T>) -> impl Fn(T) {
        let tx = Mutex::new(Some(tx));
        move |v| {
            if let Some(tx) = tx.lock().ok().and_then(|mut t| t.take()) {
                let _ = tx.send(v);
            }
        }
    }

    /// Asks macOS for what it hasn't been asked yet (its prompts wait for your answer). The blocks aren't `Send`, so
    /// each is gone before the wait.
    async fn ask() {
        if speech_permission() == SFSpeechRecognizerAuthorizationStatus::NotDetermined {
            let rx = {
                let (tx, rx) = oneshot::channel();
                let send = once(tx);
                let block = RcBlock::new(move |_: SFSpeechRecognizerAuthorizationStatus| send(()));
                // SAFETY: the callee copies the block and calls it once, on any thread.
                unsafe { SFSpeechRecognizer::requestAuthorization(&block) };
                rx
            };
            let _ = rx.await;
        }
        if available!(macos = 14.0)
            && mic_permission() == AVAudioApplicationRecordPermission::Undetermined
        {
            let rx = {
                let (tx, rx) = oneshot::channel();
                let send = once(tx);
                let block = RcBlock::new(move |_: Bool| send(()));
                // SAFETY: as above.
                unsafe { AVAudioApplication::requestRecordPermissionWithCompletionHandler(&block) };
                rx
            };
            let _ = rx.await;
        }
    }

    /// Opens the microphone and starts hearing, after asking macOS if it never has; errs with why it can't.
    pub async fn start(hear: Hear, hints: Vec<String>) -> Result<()> {
        if status().state == SpeechState::Ask {
            ask().await;
        }
        let now = status();
        if now.state != SpeechState::Ready {
            return Err(AppError::other(now.reason.unwrap_or_default()));
        }
        cancel();
        let (ready_tx, ready_rx) = oneshot::channel();
        let (end_tx, end_rx) = mpsc::channel();
        let (done_tx, done_rx) = oneshot::channel();
        std::thread::Builder::new()
            .name("gliide-speech".into())
            .spawn(move || run(hear, hints, ready_tx, end_rx, done_tx))
            .map_err(|e| AppError::other(format!("Couldn’t open the microphone: {e}")))?;
        ready_rx
            .await
            .map_err(|_| AppError::other("Couldn’t open the microphone"))??;
        *lock(&SESSION) = Some(Session {
            end: end_tx,
            done: done_rx,
        });
        Ok(())
    }

    /// Closes the microphone and returns what was heard.
    pub async fn stop() -> Result<Heard> {
        let Some(s) = lock(&SESSION).take() else {
            return Err(AppError::other("The microphone isn’t open"));
        };
        let _ = s.end.send(End::Stop);
        s.done
            .await
            .map_err(|_| AppError::other("The microphone closed without an answer"))?
    }

    /// Closes the microphone and drops what was heard.
    pub fn cancel() {
        if let Some(s) = lock(&SESSION).take() {
            let _ = s.end.send(End::Cancel);
        }
    }

    /// What the recognizer's handler shares with the session's thread.
    struct Shared {
        transcript: Mutex<Transcript>,
        /// You closed the microphone: the result that follows is expected, not news.
        closing: AtomicBool,
        /// Told once the recognizer finished (a final result or an error).
        finished: Mutex<Option<mpsc::Sender<()>>>,
    }

    /// One session, on its own thread: open, wait for its end, close, and hand back what was heard.
    fn run(
        hear: Hear,
        hints: Vec<String>,
        ready: oneshot::Sender<Result<()>>,
        end: mpsc::Receiver<End>,
        done: oneshot::Sender<Result<Heard>>,
    ) {
        autoreleasepool(|_| {
            let (finished_tx, finished_rx) = mpsc::channel();
            let shared = Arc::new(Shared {
                transcript: Mutex::new(Transcript::default()),
                closing: AtomicBool::new(false),
                finished: Mutex::new(Some(finished_tx)),
            });
            let open = match open(&hear, &hints, &shared) {
                Ok(open) => open,
                Err(e) => {
                    let _ = ready.send(Err(e));
                    return;
                }
            };
            let started = Instant::now();
            let how = if ready.send(Ok(())).is_err() {
                End::Cancel
            } else {
                match end.recv_timeout(MAX) {
                    Ok(how) => how,
                    Err(RecvTimeoutError::Timeout) => {
                        let text = lock(&shared.transcript).text();
                        hear(Hearing {
                            text,
                            stopped: true,
                        });
                        // The UI collects it (stop) or drops it.
                        end.recv().unwrap_or(End::Cancel)
                    }
                    Err(RecvTimeoutError::Disconnected) => End::Cancel,
                }
            };
            let ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
            shared.closing.store(true, Ordering::SeqCst);
            open.close();
            match how {
                // SAFETY: a plain call on an object this thread owns.
                End::Cancel => unsafe { open.task.cancel() },
                End::Stop => {
                    // SAFETY: as above.
                    unsafe { open.request.endAudio() };
                    let _ = finished_rx.recv_timeout(FINISH);
                    // SAFETY: as above; finishing a task that already ended does nothing.
                    unsafe { open.task.finish() };
                    let text = lock(&shared.transcript).text();
                    let words = words(&text);
                    let _ = done.send(Ok(Heard {
                        text,
                        spoken: Spoken { ms, words },
                    }));
                }
            }
        });
    }

    type TapBlock = RcBlock<dyn Fn(NonNull<AVAudioPCMBuffer>, NonNull<AVAudioTime>)>;
    type ResultBlock = RcBlock<dyn Fn(*mut SFSpeechRecognitionResult, *mut NSError)>;

    /// The engine running with its tap feeding the request, and the task hearing it. The blocks live as long as it.
    struct Open {
        engine: Retained<AVAudioEngine>,
        request: Retained<SFSpeechAudioBufferRecognitionRequest>,
        task: Retained<SFSpeechRecognitionTask>,
        _tap: TapBlock,
        _handler: ResultBlock,
    }

    impl Open {
        /// Closes the microphone.
        fn close(&self) {
            // SAFETY: plain calls on objects this thread owns; the tap goes before its block does.
            unsafe {
                self.engine.stop();
                self.engine.inputNode().removeTapOnBus(0);
            }
        }
    }

    fn open(hear: &Hear, hints: &[String], shared: &Arc<Shared>) -> Result<Open> {
        let recognizer = recognizer().ok_or_else(|| AppError::other(NO_ON_DEVICE))?;
        let hints: Vec<Retained<NSString>> = hints
            .iter()
            .map(|h| h.trim())
            .filter(|h| !h.is_empty())
            .take(MAX_HINTS)
            .map(NSString::from_str)
            .collect();
        // SAFETY: object creation and property writes on this thread. The handler runs on a queue of its own, off the
        // main thread, which the window's event loop owns.
        let request = unsafe {
            recognizer.setQueue(&NSOperationQueue::new());
            let request = SFSpeechAudioBufferRecognitionRequest::new();
            request.setShouldReportPartialResults(true);
            request.setRequiresOnDeviceRecognition(true);
            request.setTaskHint(SFSpeechRecognitionTaskHint::Dictation);
            if available!(macos = 13.0) {
                request.setAddsPunctuation(true);
            }
            request.setContextualStrings(&NSArray::from_retained_slice(&hints));
            request
        };
        // SAFETY: object creation and property reads on this thread.
        let (engine, input, format) = unsafe {
            let engine = AVAudioEngine::new();
            let input = engine.inputNode();
            let format = input.outputFormatForBus(0);
            (engine, input, format)
        };
        // SAFETY: property reads.
        if unsafe { format.sampleRate() <= 0.0 || format.channelCount() == 0 } {
            return Err(AppError::other("There’s no microphone to hear you with"));
        }
        let feed = request.clone();
        let tap: TapBlock = RcBlock::new(
            move |buffer: NonNull<AVAudioPCMBuffer>, _: NonNull<AVAudioTime>| {
                // SAFETY: AVFAudio passes a valid buffer for the block's duration; the request is fed from the audio
                // thread, as it's meant to be.
                unsafe { feed.appendAudioPCMBuffer(buffer.as_ref()) };
            },
        );
        // SAFETY: the block outlives the tap (`Open::close` removes it first); the format is the input's own.
        let started = unsafe {
            input.installTapOnBus_bufferSize_format_block(
                0,
                1024,
                Some(&format),
                RcBlock::as_ptr(&tap),
            );
            engine.prepare();
            engine.startAndReturnError()
        };
        if let Err(e) = started {
            // SAFETY: as above.
            unsafe { input.removeTapOnBus(0) };
            return Err(AppError::other(format!(
                "Couldn’t open the microphone: {}",
                e.localizedDescription()
            )));
        }
        let (h, s) = (Arc::clone(hear), Arc::clone(shared));
        let handler: ResultBlock = RcBlock::new(
            move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
                // SAFETY: a non-null result is valid for the block's duration.
                let (text, last) = match unsafe { result.as_ref() } {
                    Some(r) => {
                        let said = unsafe { r.bestTranscription().formattedString() }.to_string();
                        let mut t = lock(&s.transcript);
                        t.heard(&said);
                        (t.text(), unsafe { r.isFinal() })
                    }
                    None => (lock(&s.transcript).text(), false),
                };
                let ended = last || !error.is_null();
                // Heard while you speak, and its end when it ends on its own; once you close it, `stop` answers.
                if !s.closing.load(Ordering::SeqCst) {
                    h(Hearing {
                        text,
                        stopped: ended,
                    });
                }
                if ended && let Some(tx) = lock(&s.finished).take() {
                    let _ = tx.send(());
                }
            },
        );
        // SAFETY: the request and the handler live as long as the task (`Open`); the handler runs on the queue above.
        let task =
            unsafe { recognizer.recognitionTaskWithRequest_resultHandler(&request, &handler) };
        Ok(Open {
            engine,
            request,
            task,
            _tap: tap,
            _handler: handler,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refining_replaces_and_starting_over_keeps_what_came_before() {
        let mut t = Transcript::default();
        t.heard("I led the");
        t.heard("I led the migration to Postgres");
        assert_eq!(t.text(), "I led the migration to Postgres");
        // A refinement reworks words without losing half of them: still the same telling.
        t.heard("Hi led the migration to Postgres.");
        assert_eq!(t.text(), "Hi led the migration to Postgres.");
        // After a pause the recognizer starts over: the earlier telling stays, whatever word it starts with.
        t.heard("It");
        t.heard("It took three weeks.");
        assert_eq!(
            t.text(),
            "Hi led the migration to Postgres. It took three weeks."
        );
        t.heard("I");
        t.heard("I owned the rollback plan.");
        assert_eq!(
            t.text(),
            "Hi led the migration to Postgres. It took three weeks. I owned the rollback plan."
        );
    }

    #[test]
    fn words_count_what_was_said() {
        assert_eq!(words("  I led the   migration. "), 4);
        assert_eq!(words(""), 0);
    }
}
