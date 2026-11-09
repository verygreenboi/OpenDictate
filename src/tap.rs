//! System audio capture through a Core Audio process tap.
//!
//! The microphone alone is one side of a call. A tap records what the Mac is
//! playing, which is everyone else, and unlike a virtual audio driver it
//! needs nothing installed, and unlike ScreenCaptureKit it never asks to
//! record the screen.
//!
//! Needs macOS 14.4 and the System Audio Recording permission, which macOS
//! asks for at the first `AudioDeviceStart` rather than at launch. Every call
//! is checked, so on an older system this fails at the first one and the
//! caller falls back to the microphone.

use objc2::rc::Retained;
use objc2::AnyThread;
use objc2_core_audio::*;
use objc2_core_audio_types::{AudioBufferList, AudioStreamBasicDescription, AudioTimeStamp};
use objc2_core_foundation::{CFDictionary, CFString};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString};
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

/// Everything the IO proc needs. Handed over as the client pointer, and
/// reclaimed when the capture stops.
struct Sink {
    /// Mono, at the device's own rate: every channel of every frame averaged,
    /// which is what mixes the microphone and the tap into one voice track.
    samples: Mutex<Vec<f32>>,
}

pub struct SystemCapture {
    tap: AudioObjectID,
    device: AudioObjectID,
    proc_id: AudioDeviceIOProcID,
    sink: *mut Arc<Sink>,
    pub rate: f64,
    /// Whether the microphone made it into the same device, or only the tap.
    pub has_mic: bool,
}

// The Core Audio ids are plain integers and the sink is reference counted;
// nothing here is tied to the thread that opened it.
unsafe impl Send for SystemCapture {}

fn ns(s: &str) -> Retained<NSString> {
    NSString::from_str(s)
}

fn key(k: &std::ffi::CStr) -> Retained<NSString> {
    ns(k.to_str().unwrap_or_default())
}

unsafe fn get<T>(object: AudioObjectID, selector: AudioObjectPropertySelector, out: &mut T) -> i32 {
    let a = AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };
    let mut size = std::mem::size_of::<T>() as u32;
    AudioObjectGetPropertyData(
        object,
        NonNull::from(&a),
        0,
        std::ptr::null(),
        NonNull::from(&mut size),
        NonNull::new(out as *mut T as *mut c_void).unwrap(),
    )
}

unsafe extern "C-unwind" fn io_proc(
    _device: AudioObjectID,
    _now: NonNull<AudioTimeStamp>,
    input: NonNull<AudioBufferList>,
    _input_time: NonNull<AudioTimeStamp>,
    _output: NonNull<AudioBufferList>,
    _output_time: NonNull<AudioTimeStamp>,
    client: *mut c_void,
) -> i32 {
    if client.is_null() {
        return 0;
    }
    let sink = &*(client as *const Arc<Sink>);
    let list = input.as_ref();
    let buffers = std::slice::from_raw_parts(list.mBuffers.as_ptr(), list.mNumberBuffers as usize);

    // Frames are the same across buffers; channels are split across them when
    // the device is non-interleaved and packed inside them when it is not.
    let mut frames = 0usize;
    let mut channels = 0usize;
    for b in buffers {
        if b.mData.is_null() || b.mNumberChannels == 0 {
            continue;
        }
        let n = b.mDataByteSize as usize / 4 / b.mNumberChannels as usize;
        frames = frames.max(n);
        channels += b.mNumberChannels as usize;
    }
    if frames == 0 || channels == 0 {
        return 0;
    }

    let mut mono = vec![0f32; frames];
    for b in buffers {
        if b.mData.is_null() || b.mNumberChannels == 0 {
            continue;
        }
        let ch = b.mNumberChannels as usize;
        let n = (b.mDataByteSize as usize / 4 / ch).min(frames);
        let data = std::slice::from_raw_parts(b.mData as *const f32, n * ch);
        for (f, slot) in mono.iter_mut().enumerate().take(n) {
            let mut sum = 0f32;
            for c in 0..ch {
                sum += data[f * ch + c];
            }
            *slot += sum;
        }
    }
    let scale = 1.0 / channels as f32;
    if let Ok(mut out) = sink.samples.lock() {
        out.extend(mono.iter().map(|s| s * scale));
    }
    0
}

/// An OSStatus as the four-character code Core Audio means by it.
fn status(st: i32) -> String {
    let b = (st as u32).to_be_bytes();
    if b.iter().all(|c| c.is_ascii_graphic()) {
        format!("{st} '{}'", String::from_utf8_lossy(&b))
    } else {
        st.to_string()
    }
}

unsafe fn device_uid(device: AudioObjectID) -> String {
    let mut uid: *const CFString = std::ptr::null();
    get(device, kAudioDevicePropertyDeviceUID, &mut uid);
    if uid.is_null() { String::new() } else { (*uid).to_string() }
}

/// The Core Audio id of the system's default input, for putting the
/// microphone inside the same device as the tap.
///
/// The app's own microphone setting names a cpal device, and mapping that
/// name onto a Core Audio id is a job of its own, so a meeting records
/// whichever input the system is set to.
pub fn default_input() -> Option<AudioObjectID> {
    unsafe {
        let mut dev: AudioObjectID = 0;
        let st = get(kAudioObjectSystemObject as u32, kAudioHardwarePropertyDefaultInputDevice, &mut dev);
        (st == 0 && dev != 0).then_some(dev)
    }
}

impl SystemCapture {
    /// Opens one device carrying the system's output and, when `mic` names a
    /// Core Audio device, that microphone with it. Both arrive mixed, which
    /// is what a meeting transcript wants.
    pub fn start(mic: Option<AudioObjectID>) -> Result<Self, String> {
        unsafe {
            let desc = CATapDescription::initStereoGlobalTapButExcludeProcesses(
                CATapDescription::alloc(),
                &NSArray::new(),
            );
            desc.setName(&ns("Open Dictate"));
            desc.setPrivate(true);
            // Unmuted: a meeting you cannot hear is not a meeting.
            desc.setMuteBehavior(CATapMuteBehavior::Unmuted);

            let mut tap: AudioObjectID = 0;
            let st = AudioHardwareCreateProcessTap(Some(&desc), &mut tap);
            if st != 0 || tap == 0 {
                return Err(format!("system audio is unavailable (tap {})", status(st)));
            }
            let uuid = desc.UUID().UUIDString().to_string();

            let mut out_dev: AudioObjectID = 0;
            get(kAudioObjectSystemObject as u32, kAudioHardwarePropertyDefaultOutputDevice, &mut out_dev);
            let out_uid = device_uid(out_dev);

            let mic_uid = mic.map(|d| device_uid(d)).filter(|u| !u.is_empty());
            let has_mic = mic_uid.is_some();
            let subs: Vec<Retained<NSDictionary<NSString, objc2::runtime::AnyObject>>> = mic_uid
                .iter()
                .map(|uid| {
                    NSDictionary::from_slices::<NSString>(
                        &[&key(kAudioSubDeviceUIDKey), &key(kAudioSubDeviceDriftCompensationKey)],
                        &[&*ns(uid) as &objc2::runtime::AnyObject, &*NSNumber::new_u32(1)],
                    )
                })
                .collect();
            let sub_refs: Vec<&objc2::runtime::AnyObject> =
                subs.iter().map(|d| &**d as &objc2::runtime::AnyObject).collect();

            let sub_tap = NSDictionary::from_slices::<NSString>(
                &[&key(kAudioSubTapUIDKey), &key(kAudioSubTapDriftCompensationKey)],
                &[&*ns(&uuid) as &objc2::runtime::AnyObject, &*NSNumber::new_u32(1)],
            );
            let agg = NSDictionary::from_slices::<NSString>(
                &[
                    &key(kAudioAggregateDeviceNameKey),
                    &key(kAudioAggregateDeviceUIDKey),
                    &key(kAudioAggregateDeviceMainSubDeviceKey),
                    &key(kAudioAggregateDeviceIsPrivateKey),
                    &key(kAudioAggregateDeviceTapAutoStartKey),
                    &key(kAudioAggregateDeviceSubDeviceListKey),
                    &key(kAudioAggregateDeviceTapListKey),
                ],
                &[
                    &*ns("Open Dictate meeting") as &objc2::runtime::AnyObject,
                    &*ns("app.opendictate.meeting.aggregate"),
                    &*ns(&out_uid),
                    &*NSNumber::new_u32(1),
                    &*NSNumber::new_u32(1),
                    &*NSArray::from_slice(&sub_refs),
                    &*NSArray::from_slice(&[&*sub_tap]),
                ],
            );
            let mut device: AudioObjectID = 0;
            let st = AudioHardwareCreateAggregateDevice(
                &*(Retained::as_ptr(&agg) as *const CFDictionary),
                NonNull::from(&mut device),
            );
            if st != 0 || device == 0 {
                AudioHardwareDestroyProcessTap(tap);
                return Err(format!("system audio is unavailable (device {})", status(st)));
            }

            // The tap's own format is not what the IO proc delivers: an
            // aggregate device built around a microphone adopts a rate of
            // its own, and reading the tap's instead had a 42 second meeting
            // arriving as 21 seconds of audio.
            let mut rate: f64 = 0.0;
            get(device, kAudioDevicePropertyNominalSampleRate, &mut rate);
            if rate <= 0.0 {
                let mut fmt: AudioStreamBasicDescription = std::mem::zeroed();
                get(tap, kAudioTapPropertyFormat, &mut fmt);
                rate = if fmt.mSampleRate > 0.0 { fmt.mSampleRate } else { 48_000.0 };
            }

            let sink = Box::into_raw(Box::new(Arc::new(Sink { samples: Mutex::new(Vec::new()) })));
            let mut proc_id: AudioDeviceIOProcID = None;
            let st = AudioDeviceCreateIOProcID(device, Some(io_proc), sink as *mut c_void, NonNull::from(&mut proc_id));
            if st != 0 {
                drop(Box::from_raw(sink));
                AudioHardwareDestroyAggregateDevice(device);
                AudioHardwareDestroyProcessTap(tap);
                return Err(format!("system audio is unavailable (io proc {})", status(st)));
            }
            // macOS asks for System Audio Recording here, not at launch, so
            // this is the call that can sit behind a dialog.
            let st = AudioDeviceStart(device, proc_id);
            if st != 0 {
                if let Some(p) = proc_id {
                    AudioDeviceDestroyIOProcID(device, Some(p));
                }
                drop(Box::from_raw(sink));
                AudioHardwareDestroyAggregateDevice(device);
                AudioHardwareDestroyProcessTap(tap);
                return Err(format!("system audio was refused (start {})", status(st)));
            }
            Ok(Self { tap, device, proc_id, sink, rate, has_mic })
        }
    }

    /// Stops the IO proc without tearing the device down, so the last of
    /// the audio can be collected without more arriving behind it.
    pub fn pause(&self) {
        unsafe {
            AudioDeviceStop(self.device, self.proc_id);
        }
    }

    /// Everything captured since the last call, mono at `rate`.
    pub fn drain(&self) -> Vec<f32> {
        unsafe {
            let sink = &*self.sink;
            sink.samples.lock().map(|mut s| std::mem::take(&mut *s)).unwrap_or_default()
        }
    }
}

impl Drop for SystemCapture {
    fn drop(&mut self) {
        unsafe {
            AudioDeviceStop(self.device, self.proc_id);
            if let Some(p) = self.proc_id {
                AudioDeviceDestroyIOProcID(self.device, Some(p));
            }
            AudioHardwareDestroyAggregateDevice(self.device);
            AudioHardwareDestroyProcessTap(self.tap);
            drop(Box::from_raw(self.sink));
        }
    }
}

/// `--tap-check`: proves system audio works on this Mac, the way
/// `--mic-check` proves the microphone does.
///
/// Plays a system sound at itself, so a tap that runs but captures nothing
/// can be told from one that works. Only a run from the installed bundle
/// means anything about the permission: a bare binary inherits the TCC
/// identity of whatever launched it.
pub fn check(secs: u64) -> String {
    let mut r = String::new();
    match SystemCapture::start(None) {
        Err(e) => r.push_str(&format!("{e}\nVERDICT: no system audio.\n")),
        Ok(cap) => {
            r.push_str(&format!("opened at {} Hz\n", cap.rate));
            let player = std::thread::spawn(move || {
                for _ in 0..secs.max(1) {
                    let _ = std::process::Command::new("afplay")
                        .arg("/System/Library/Sounds/Submarine.aiff")
                        .status();
                }
            });
            std::thread::sleep(std::time::Duration::from_secs(secs));
            let _ = player.join();
            let s = cap.drain();
            let peak = s.iter().fold(0f32, |m, x| m.max(x.abs()));
            r.push_str(&format!("captured {} samples, peak {peak:.4}\n", s.len()));
            r.push_str(if s.is_empty() {
                "VERDICT: nothing arrived - the IO proc never ran.\n"
            } else if peak < 0.001 {
                "VERDICT: silence - running, but capturing nothing. Permission.\n"
            } else {
                "VERDICT: system audio captured.\n"
            });
        }
    }
    r
}
