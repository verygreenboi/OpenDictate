//! System audio capture through a Core Audio process tap, and the diagnostic
//! that proves it works from the signed bundle.
//!
//! The microphone alone is one side of a call. A tap records what the Mac is
//! playing, which is everyone else, and unlike a virtual audio driver it
//! needs nothing installed, and unlike ScreenCaptureKit it does not ask to
//! record the screen.
//!
//! macOS 14.4 and later. Every call is checked, so on anything older this
//! fails at the first one and the caller falls back to the microphone.

use objc2::rc::Retained;
use objc2::AnyThread;
use objc2_core_audio::*;
use objc2_core_audio_types::{AudioBufferList, AudioStreamBasicDescription, AudioTimeStamp};
use objc2_core_foundation::{CFDictionary, CFString};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString};
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::Mutex;

static CAPTURED: Mutex<Vec<f32>> = Mutex::new(Vec::new());

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
    _client: *mut c_void,
) -> i32 {
    let list = input.as_ref();
    let buffers = std::slice::from_raw_parts(list.mBuffers.as_ptr(), list.mNumberBuffers as usize);
    if let Ok(mut sink) = CAPTURED.lock() {
        for b in buffers {
            if b.mData.is_null() {
                continue;
            }
            let n = b.mDataByteSize as usize / 4;
            sink.extend_from_slice(std::slice::from_raw_parts(b.mData as *const f32, n));
        }
    }
    0
}

/// An OSStatus as the four-character code Core Audio actually means by it.
fn status(st: i32) -> String {
    let b = (st as u32).to_be_bytes();
    if st == 0 {
        "ok".into()
    } else if b.iter().all(|c| c.is_ascii_graphic()) {
        format!("{st} '{}'", String::from_utf8_lossy(&b))
    } else {
        st.to_string()
    }
}

/// Records what the Mac is playing for `secs`, with a system sound played at
/// it so silence can be told from a tap that runs but captures nothing.
///
/// Returns a report, written where `--tap-check` can read it back. The point
/// is the permission: a bare binary inherits the TCC identity of whatever
/// launched it, so this only means anything run from the installed bundle.
pub fn check(secs: u64) -> String {
    let mut r = String::new();
    let mut line = |s: String| {
        r.push_str(&s);
        r.push('\n');
    };
    CAPTURED.lock().map(|mut c| c.clear()).ok();

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
        line(format!("create tap            {}  id {tap}", status(st)));
        if st != 0 || tap == 0 {
            line("VERDICT: the tap could not be created.".into());
            return r;
        }
        let uuid = desc.UUID().UUIDString().to_string();

        let mut out_dev: AudioObjectID = 0;
        get(kAudioObjectSystemObject as u32, kAudioHardwarePropertyDefaultOutputDevice, &mut out_dev);
        let mut uid_ref: *const CFString = std::ptr::null();
        get(out_dev, kAudioDevicePropertyDeviceUID, &mut uid_ref);
        let out_uid = if uid_ref.is_null() { String::new() } else { (*uid_ref).to_string() };
        line(format!("default output        {out_dev}  {out_uid}"));

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
                &*NSArray::<objc2::runtime::AnyObject>::new(),
                &*NSArray::from_slice(&[&*sub_tap]),
            ],
        );
        let mut device: AudioObjectID = 0;
        let st = AudioHardwareCreateAggregateDevice(
            &*(Retained::as_ptr(&agg) as *const CFDictionary),
            NonNull::from(&mut device),
        );
        line(format!("create aggregate      {}  id {device}", status(st)));
        if st != 0 || device == 0 {
            AudioHardwareDestroyProcessTap(tap);
            line("VERDICT: the aggregate device could not be created.".into());
            return r;
        }

        let mut fmt: AudioStreamBasicDescription = std::mem::zeroed();
        get(tap, kAudioTapPropertyFormat, &mut fmt);
        line(format!("tap format            {} Hz, {} ch, {} bit", fmt.mSampleRate, fmt.mChannelsPerFrame, fmt.mBitsPerChannel));

        let mut proc_id: AudioDeviceIOProcID = None;
        let st = AudioDeviceCreateIOProcID(device, Some(io_proc), std::ptr::null_mut(), NonNull::from(&mut proc_id));
        line(format!("create io proc        {}", status(st)));
        // The permission prompt is reported to appear here rather than at
        // tap creation, so this is the call that matters.
        let st = AudioDeviceStart(device, proc_id);
        line(format!("start device          {}", status(st)));

        let player = std::thread::spawn(move || {
            for _ in 0..secs.max(1) {
                let _ = std::process::Command::new("afplay")
                    .arg("/System/Library/Sounds/Submarine.aiff")
                    .status();
            }
        });
        std::thread::sleep(std::time::Duration::from_secs(secs));
        let _ = player.join();

        AudioDeviceStop(device, proc_id);
        if let Some(p) = proc_id {
            AudioDeviceDestroyIOProcID(device, Some(p));
        }
        AudioHardwareDestroyAggregateDevice(device);
        AudioHardwareDestroyProcessTap(tap);
    }

    let samples = CAPTURED.lock().map(|c| c.clone()).unwrap_or_default();
    let peak = samples.iter().fold(0f32, |m, s| m.max(s.abs()));
    line(format!("captured              {} samples, peak {peak:.4}", samples.len()));
    line(
        if samples.is_empty() {
            "VERDICT: nothing arrived - the IO proc never ran."
        } else if peak < 0.001 {
            "VERDICT: silence - running, but capturing nothing. Permission."
        } else {
            "VERDICT: system audio captured."
        }
        .into(),
    );
    r
}
