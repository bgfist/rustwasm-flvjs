/*
 * Copyright (C) 2016 Bilibili. All Rights Reserved.
 *
 * @author zheng qian <xqq@xqq.im>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
use super::amf_parser::{parseScriptData, AMFValue, ScriptData};
use super::demux_errors;
use crate::core::media_info::MediaInfo;
use crate::io::loader::Loader;
use crate::remux::mp4_muxer::MP4Remuxer;
use crate::utils::{browser::Browser, logger::Log};
use crate::web_sys_wrappers::interval::Interval;
use byteorder::{BigEndian, ReadBytesExt};
use js_sys::{Date, Function, Math};
use std::collections::HashMap;
use std::io::Cursor;
use std::io::Read;
use wasm_bindgen::prelude::*;

const mpegAudioV10SampleRateTable: &[u8] = &[44100, 48000, 32000, 0];
const mpegAudioV20SampleRateTable: &[u8] = &[22050, 24000, 16000, 0];
const mpegAudioV25SampleRateTable: &[u8] = &[11025, 12000, 8000, 0];

const mpegAudioL1BitRateTable: &[u8] = &[
    0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448, -1,
];
const mpegAudioL2BitRateTable: &[u8] = &[
    0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384, -1,
];
const mpegAudioL3BitRateTable: &[u8] = &[
    0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, -1,
];

const flvSoundRateTable: &[u32] = &[5500, 11025, 22050, 44100, 48000];

const mpegSamplingRates: &[u8] = &[
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350,
];

struct KeyframesIndex {
    times: Vec<f64>,
    filepositions: Vec<f64>,
}

pub struct VideoTrack {
    _type: &'static str,
    sequenceNumber: i32,
    id: i32,
    samples: Vec<Sample>,
    length: i32,
}

pub struct VideoSample {
    pub units: Vec<Unit>,
    length: u32,
    isKeyframe: bool,
    dts: u32,
    cts: i32,
    pts: u32,
    fileposition: Option<u32>,
}

pub struct AudioTrack {
    _type: &'static str,
    sequenceNumber: i32,
    id: i32,
    samples: Vec<Sample>,
    length: i32,
}

pub struct AudioSample {
    unit: Vec<u8>,
    length: usize,
    dts: u32,
    pts: u32,
    units: (),
    isKeyframe: (),
    cts: (),
}

pub struct AudioTrackMetaData {
    id: i32,
    data: Vec<u8>,
    timescale: u32,
    duration: u32,
    codec: String,
    audioSampleRate: f64,
    channelCount: usize,
}

pub struct VideoTrackMetaData {
    id: u32,
    timescale: u32,
    duration: u32,
    codecWidth: u32,
    codecHeight: u32,
    presentWidth: u32,
    presentHeight: u32,
    profile: u32,
    level: u32,
    bitDepth: u32,
    chromaFormat: u32,
    sarRatio: u32,
    frameRate: u32,
    refSampleDuration: u32,
    codec: String,
    avcc: Vec<u8>,
}

pub enum TrackMetaData {
    Video(VideoTrackMetaData),
    Audio(AudioTrackMetaData),
}

struct AudioConfig {
    config: (),
    bitRate: u32,
    samplingRate: u32,
    channelCount: u32,
    codec: &'static str,
    originalCodec: &'static str,
}

pub enum AudioData {
    Config(AudioConfig),
    Data(Vec<u8>),
}

struct FLVProbeData {
    consumed: usize,
    dataOffset: usize,
    hasAudioTrack: bool,
    hasVideoTrack: bool,
}

struct Unit {
    _type: u32,
    data: Vec<u8>,
}

struct VideoDimension {
    width: usize,
    height: usize,
    profile: String,
    level: String,
}

struct FrameRate {
    fixed: bool,
    fps: f64,
    fps_num: usize,
    fps_den: usize,
}

struct BpsInfo {
    lastVideoBytes: usize,
    lastAudioBytes: usize,
    bps_video: usize,
    bps_audio: usize,
    data_updated_time: usize,
    time_gap_factor: usize,
}

struct Config {}

type MetaCallback = fn(this: &MP4Remuxer, meta: TrackMetaData);
type DataCallback = fn(this: &MP4Remuxer, audioTrack: &mut AudioTrack, videoTrack: &mut VideoTrack);

pub struct FLVDemuxer {
    TAG: &'static str,
    _config: Config,
    _mediaInfo: MediaInfo,

    firstParse: bool,
    _hasAudio: bool,
    _hasVideo: bool,
    _hasAudioFlagOverrided: bool,
    _hasVideoFlagOverrided: bool,
    _audioInitialMetadataDispatched: bool,
    _videoInitialMetadataDispatched: bool,

    _lastVideoDimension: Option<VideoDimension>,

    _dispatch: bool,
    _firstParse: bool,
    _metadata: Option<ScriptData>,
    _audioMetadata: Option<AudioTrackMetaData>,
    _videoMetadata: Option<VideoTrackMetaData>,
    _naluLengthSize: i32,
    _timestampBase: i32,
    _timescale: i32,
    _duration: i32,
    _durationOverrided: bool,
    _bpsCalculator: Option<Interval>,
    _dataOffset: u32,
    _referenceFrameRate: FrameRate,
    _bpsInfo: Option<BpsInfo>,

    _now: Function,

    _videoTrack: VideoTrack,
    _audioTrack: AudioTrack,
    _onTrackMetadata: Option<MetaCallback>,
    _onDataAvailable: Option<DataCallback>,
    _onMetaDataArrived: Option<Function>,
    _onMediaInfo: Option<Function>,
    _onError: Option<Function>,
}

impl FLVDemuxer {
    fn new(probeData: FLVProbeData, config: Config) -> FLVDemuxer {
        FLVDemuxer {
            TAG: "FLVDemuxer",
            _config: config,

            _mediaInfo: MediaInfo::default(),
            firstParse: (),

            _hasAudio: probeData.hasAudioTrack,
            _hasVideo: probeData.hasVideoTrack,
            _hasAudioFlagOverrided: false,

            _hasVideoFlagOverrided: false,
            _audioInitialMetadataDispatched: false,
            _videoInitialMetadataDispatched: false,
            _lastVideoDimension: VideoDimension {
                width: -1,
                height: -1,
                profile: "",
                level: "",
            },

            _videoTrack: VideoTrack {
                _type: "video",
                id: 1,
                sequenceNumber: 0,
                samples: [],
                length: 0,
            },
            _audioTrack: AudioTrack {
                _type: "audio",
                id: 2,
                sequenceNumber: 0,
                samples: [],
                length: 0,
            },

            _dispatch: false,
            //  _mediaInfo.hasAudio : _hasAudio,
            //  _mediaInfo.hasVideo : _hasVideo,
            _dispatch: false,
            _firstParse: false,
            _firstParse: true,

            _metadata: None,
            _audioMetadata: None, // int32, in milliseconds
            _videoMetadata: None,
            _naluLengthSize: 4, // int32, in milliseconds
            _timestampBase: 0,
            _timescale: 1000,

            _duration: 0,

            _durationOverrided: false,

            _bpsCalculator: None,

            // compatibility detection
            _dataOffset: probeData.dataOffset,

            _referenceFrameRate: FrameRate {
                fixed: true,
                fps: 23.976,
                fps_num: 23976,
                fps_den: 1000,
            },
            _bpsInfo: Some(BpsInfo {
                lastVideoBytes: 0,
                lastAudioBytes: 0,
                bps_video: 0,
                bps_audio: 0,
                data_updated_time: 0,
                time_gap_factor: 1,
            }),
            _now: {
                let window = web_sys::window().unwrap();
                window
                    .performance()
                    .and_then(|performance| performance.now())
                    .or(Date::now())
            },

            _audioMetadata: None,
            _videoMetadata: None,
        }
    }

    fn destroy(&self) {
        self._mediaInfo = None;
        self._metadata = None;
        self._audioMetadata = None;
        self._videoMetadata = None;
        self._videoTrack = None;
        self._audioTrack = None;

        self._onError = None;
        self._onMediaInfo = None;
        self._onMetaDataArrived = None;
        self._onScriptDataArrived = None;
        self._onTrackMetadata = None;
        self._onDataAvailable = None;
        self._lastVideoDimension = None;
        self._bpsCalculator = None;

        // if (self._bpsCalculator) {
        //     clear_interval_with_handle(&window().unwrap(), self._bpsCalculator);
        //     self._bpsCalculator = None;
        //     self._bpsInfo = None;
        // }
    }

    /*
     * 读取FLV的header信息
     */
    fn probe(data: &[u8]) -> Option<FLVProbeData> {
        // "F L V version"
        if data[0] != 0x46 || data[1] != 0x4C || data[2] != 0x56 || data[3] != 0x01 {
            return None;
        }

        let hasAudio = ((data[4] & 4) >> 2) != 0;
        let hasVideo = (data[4] & 1) != 0;

        let offset = ReadBig32(data, 5);

        if (offset < 9) {
            return None;
        }

        Some(FLVProbeData {
            consumed: offset,
            dataOffset: offset,
            hasAudioTrack: hasAudio,
            hasVideoTrack: hasVideo,
        })
    }

    fn bindDataSource(&self, loader: Loader) -> &Self {
        loader.onDataArrival(loader, self.parseChunks);
        return self;
    }

    #[wasm_bindgen(getter, js_name = "onTrackMetadata")]
    pub fn get_onTrackMetadata(&self) -> Option<MetaCallback> {
        self._onTrackMetadata
    }

    #[wasm_bindgen(setter, js_name = "onTrackMetadata")]
    pub fn set_onTrackMetadata(&self, callback: Option<MetaCallback>) {
        self._onTrackMetadata = callback;
    }

    #[wasm_bindgen(getter, js_name = "onDataAvailable")]
    pub fn get_onDataAvailable(&self) -> Option<DataCallback> {
        self._onTrackMetadata
    }

    #[wasm_bindgen(setter, js_name = "onDataAvailable")]
    pub fn set_onDataAvailable(&self, callback: Option<DataCallback>) {
        self._onDataAvailable = callback;
    }

    fn resetMediaInfo(&self) {
        self._mediaInfo = MediaInfo::default();
    }

    fn _isInitialMetadataDispatched(&self) -> bool {
        if self._hasAudio && self._hasVideo {
            // both audio & video
            return self._audioInitialMetadataDispatched && self._videoInitialMetadataDispatched;
        }
        if self._hasAudio && !self._hasVideo {
            // audio only
            return self._audioInitialMetadataDispatched;
        }
        if (!self._hasAudio && self._hasVideo) {
            // video only
            return self._videoInitialMetadataDispatched;
        }
        return false;
    }

    fn _calculateRealtimeBitrate(&self) {
        if self._bpsInfo.time_gap_factor < 1 {
            self._bpsInfo.time_gap_factor = 1;
        }

        if self._bpsInfo.lastVideoBytes != 0 {
            self._bpsInfo.bps_video =
                8 * self._bpsInfo.lastVideoBytes / 1024 / self._bpsInfo.time_gap_factor;
            self._bpsInfo.lastVideoBytes = 0;
        }

        if self._bpsInfo.lastAudioBytes != 0 {
            self._bpsInfo.bps_audio =
                8 * self._bpsInfo.lastAudioBytes / 1024 / self._bpsInfo.time_gap_factor;
            self._bpsInfo.lastAudioBytes = 0;
        }

        // Log::d(self.TAG, "realtime av bitrate: v:"
        //     + self._bpsInfo.bps_video
        //     + ", a:" + self._bpsInfo.bps_audio
        //     + ", time_gap_factor:" + self._bpsInfo.time_gap_factor);
    }

    fn parseChunks(&self, chunk: &[u8], byteStart: usize) -> usize {
        if self._onError.is_none()
            || self._onMediaInfo.is_none()
            || self._onTrackMetadata.is_none()
            || self._onDataAvailable.is_none()
        {
            panic!("Flv: onError & onMediaInfo & onTrackMetadata & onDataAvailable callback must be specified");
        }

        let offset = 0;

        if byteStart == 0 {
            // buffer with FLV header
            if chunk.len() > 13 {
                let probeData = FLVDemuxer::probe(chunk).unwrap();
                offset = probeData.dataOffset; //跳过9个字节的FLV Header
            } else {
                return 0;
            }
        }

        self._bpsCalculator.get_or_insert(Interval::new(
            Box::new(|| self._calculateRealtimeBitrate()),
            1000,
        ));

        if self._firstParse {
            // handle PreviousTagSize0 before Tag1
            self._firstParse = false;
            if byteStart + offset != self._dataOffset {
                Log::w(self.TAG, "First time parsing but chunk byteStart invalid!");
            }

            let v = Cursor::new(&chunk[offset..]);
            let prevTagSize0 = v.read_u32::<BigEndian>().unwrap();
            if prevTagSize0 != 0 {
                Log::w(self.TAG, "PrevTagSize0 != 0 !!!");
            }
            offset += 4; //跳过PreviousTagSize0
        }

        let videoBytes: u32 = 0;
        let audioBytes = 0;

        while offset < chunk.len() {
            self._dispatch = true;

            let v = Cursor::new(&chunk[offset..]);

            // 11: Tag Header length
            if offset + 11 + 4 > chunk.len() {
                // data not enough for parsing an flv tag
                break;
            }

            let tagType = v.read_u8().unwrap();
            let dataSize = v.read_u24::<BigEndian>().unwrap();

            if offset + 11 + dataSize + 4 > chunk.len() {
                // data not enough for parsing actual data body
                break;
            }

            //8:audio, 9:video, 18(0x12):script
            if tagType != 8 && tagType != 9 && tagType != 18 {
                Log::w(self.TAG, "Unsupported tag type ${tagType}, skipped");
                // consume the whole tag (skip it)
                offset += 11 + dataSize + 4; //不认识的tag类型，offset跳过11字节tag header + data size + 4(previous tag length)，跳到下一个tag type的位置
                continue;
            }

            let ts2 = v.read_u8().unwrap() as u32;
            let ts1 = v.read_u8().unwrap() as u32;
            let ts0 = v.read_u8().unwrap() as u32;
            let ts3 = v.read_u8().unwrap() as u32;

            // 3字节时间戳 + 1字节扩展时间戳
            let timestamp = ts0 | (ts1 << 8) | (ts2 << 16) | (ts3 << 24);

            // 3字节：streamId，总是为0
            let streamId = v.read_u24::<BigEndian>().unwrap();
            if streamId != 0 {
                Log::w(self.TAG, "Meet tag which has StreamID != 0!");
            }

            let dataOffset = offset + 11; //+11跳过tag header，指向tag data

            match tagType {
                // Audio
                8 => {
                    self._parseAudioData(chunk, dataOffset, dataSize, timestamp);
                    audioBytes += dataSize;
                }
                // Video
                9 => {
                    self._parseVideoData(
                        chunk,
                        dataOffset,
                        dataSize,
                        timestamp,
                        byteStart + offset,
                    );
                    videoBytes += dataSize;
                }
                // ScriptDataObject
                18 => {
                    self._parseScriptData(chunk, dataOffset, dataSize);
                }
                _ => {
                    unreachable!();
                }
            }

            v.set_position(11 + dataSize);
            let prevTagSize = v.read_u32::<BigEndian>().unwrap();
            if prevTagSize != 11 + dataSize {
                Log::w(self.TAG, "Invalid PrevTagSize ${prevTagSize}");
            }

            offset += 11 + dataSize + 4; // tagBody + dataSize + prevTagSize
        }

        // Log::v(self.TAG, "parseChunks, byteStart=" + byteStart
        //     + ", chunk.byteLength=" + chunk.byteLength
        //     + ", audioBytes=" + audioBytes
        //     + ", videoBytes=" + videoBytes);

        self._bpsInfo.and_then(|mut bpsInfo| {
            bpsInfo.lastAudioBytes += audioBytes;
            bpsInfo.lastVideoBytes += videoBytes;

            if bpsInfo.data_updated_time != 0 {
                let factor = (self._now() - bpsInfo.data_updated_time) / 1000;
                factor = if (factor < 1)? {
                    1
                } else {
                    unsafe { Math::round(factor) }
                };
                bpsInfo.time_gap_factor = factor;
            }
            bpsInfo.data_updated_time = self._now();
        });

        if self._isInitialMetadataDispatched() {
            if self._dispatch && (self._audioTrack.length > 0 || self._videoTrack.length > 0) {
                self._onDataAvailable.unwrap()(&mut self._audioTrack, &mut self._videoTrack);
            }
        }

        return offset; // consumed bytes, just equals latest offset index
    }

    fn _parseScriptData(
        &self,
        arrayBuffer: &[u8],
        dataOffset: usize,
        dataSize: usize,
    ) -> Result<(), JsValue> {
        let scriptData = parseScriptData(arrayBuffer, dataOffset, dataSize);

        scriptData.get("onMetaData").and_then(|onMetaData| {
            if self._metadata.is_some() {
                Log::w(self.TAG, "Found another onMetaData tag!");
            }

            self._metadata = Some(scriptData);

            // self._onMetaDataArrived.and_then(|_onMetaDataArrived| {
            //     _onMetaDataArrived.call1
            // })

            if let Some(&AMFValue::Boolean(hasAudio)) = onMetaData.get("hasAudio") {
                if self._hasAudioFlagOverrided == false {
                    self._hasAudio = hasAudio;
                    self._mediaInfo.hasAudio = Some(hasAudio);
                }
            }

            if let Some(&AMFValue::Boolean(hasVideo)) = onMetaData.get("hasVideo") {
                if self._hasVideoFlagOverrided == false {
                    self._hasVideo = hasVideo;
                    self._mediaInfo.hasVideo = Some(hasVideo);
                }
            }

            if let Some(&AMFValue::Number(audiodatarate)) = onMetaData.get("audiodatarate") {
                self._mediaInfo.audioDataRate = Some(audiodatarate);
            }

            if let Some(&AMFValue::Number(videodatarate)) = onMetaData.get("videodatarate") {
                self._mediaInfo.videoDataRate = Some(videodatarate);
            }

            if let Some(&AMFValue::Number(width)) = onMetaData.get("width") {
                self._mediaInfo.width = Some(width);
            }

            if let Some(&AMFValue::Number(height)) = onMetaData.get("height") {
                self._mediaInfo.height = Some(height);
            }

            if let Some(&AMFValue::Number(duration)) = onMetaData.get("duration") {
                if !self._durationOverrided {
                    let duration = unsafe { Math::floor(duration * self._timescale) };
                    self._duration = duration;
                    self._mediaInfo.duration = Some(duration);
                }
            } else {
                self._mediaInfo.duration = Some(0);
            }

            if let Some(&AMFValue::Number(framerate)) = onMetaData.get("framerate") {
                // framerate
                let fps_num = unsafe { Math::floor(onMetaData.framerate * 1000) };
                if fps_num > 0 {
                    let fps = fps_num / 1000;
                    self._referenceFrameRate.fixed = true;
                    self._referenceFrameRate.fps = fps;
                    self._referenceFrameRate.fps_num = fps_num;
                    self._referenceFrameRate.fps_den = 1000;
                    self._mediaInfo.fps = fps;
                }
            }

            if let Some(AMFValue::Object(keyframes)) = onMetaData.remove("keyframes") {
                // keyframes
                self._mediaInfo.hasKeyframesIndex = true;
                self._mediaInfo.keyframesIndex = self._parseKeyframesIndex(keyframes);
            } else {
                self._mediaInfo.hasKeyframesIndex = false;
            }

            self._dispatch = false;
            self._mediaInfo.metadata = onMetaData;

            if self._mediaInfo.isComplete() {
                self._onMediaInfo
                    .unwrap()
                    .call1(&JsValue::null(), &JsValue::from(self._mediaInfo));
            }
        });
    }

    fn _parseKeyframesIndex(&self, keyframes: HashMap<String, AMFValue>) -> Result<usize, JsValue> {
        let times = vec![];
        let filepositions = vec![];

        if let (Some(AMFValue::Array(times)), Some(AMFValue::Array(filepositions))) =
            (keyframes.get("times"), keyframes.get("filepositions"))
        {
            for i in 1..times.len() {
                let time = self._timestampBase + unsafe { Math::floor(times[i] * 1000) };
                times.push(time);
                filepositions.push(filepositions[i]);
            }
        } else {
            panic!("flv amf keyframes format error");
        }

        return KeyframesIndex {
            times,
            filepositions,
        };
    }

    fn _parseAudioData(
        &self,
        arrayBuffer: &[u8],
        dataOffset: usize,
        dataSize: usize,
        tagTimestamp: u32,
    ) {
        if dataSize <= 1 {
            Log::w(
                self.TAG,
                "Flv: Invalid audio packet, missing SoundData payload!",
            );
            return;
        }

        if self._hasAudioFlagOverrided == true && self._hasAudio == false {
            // If hasAudio: false indicated explicitly in MediaDataSource,
            // Ignore all the audio packets
            return;
        }

        let v = Cursor::new(arrayBuffer[dataOffset..]);

        let soundSpec = v.read_u8().unwrap();

        let soundFormat = soundSpec >> 4;
        if soundFormat != 2 && soundFormat != 10 {
            // MP3 or AAC
            self._onError.unwrap().call(
                &JsValue::null(),
                &JsValue::from(demux_errors::CODEC_UNSUPPORTED),
                &JsValue::from(format!(
                    "{}{}",
                    "Flv: Unsupported audio codec idx: ", soundFormat
                )),
            );
            return;
        }

        let soundRate = 0;
        let soundRateIndex = (soundSpec & 0b00001100) >> 2;

        if soundRateIndex >= 0 && soundRateIndex <= 4 {
            soundRate = flvSoundRateTable[soundRateIndex];
        } else {
            self._onError.unwrap().call2(
                &JsValue::null(),
                &JsValue::from(demux_errors::FORMAT_ERROR),
                &JsValue::from(format!(
                    "{}{}",
                    "Flv: Invalid audio sample rate idx: ", soundRateIndex
                )),
            );
            return;
        }

        let soundSize = (soundSpec & 2) >> 1; // unused
        let soundType = soundSpec & 1;

        let meta = self._audioMetadata;
        let track = self._audioTrack;

        if meta.is_none() {
            if self._hasAudio == false && self._hasAudioFlagOverrided == false {
                self._hasAudio = true;
                self._mediaInfo.hasAudio = true;
            }

            // initial metadata
            meta = self._audioMetadata = Some(TrackMetaData::Audio(AudioTrackMetaData {
                id: track.id,
                timescale: self._timescale,
                duration: self._duration,
                audioSampleRate: soundRate,
                channelCount: if soundType == 0 { 1 } else { 2 },
                data: vec![],
                codec: "".into(),
            }));
        }

        let meta = meta.unwrap();

        if soundFormat == 10 {
            // AAC
            let aacData = self._parseAACAudioData(arrayBuffer, dataOffset + 1, dataSize - 1);
            if (aacData.is_none()) {
                return;
            }

            let aacData = aacData.unwrap();

            if let AudioData::Config(misc) = aacData {
                // AAC sequence header (AudioSpecificConfig)
                if meta.config {
                    Log::w(self.TAG, "Found another AudioSpecificConfig!");
                }

                if (meta.channelCount == misc.channelCount
                    && meta.audioSampleRate == misc.samplingRate
                    && meta.codec == misc.codec
                    && meta.originalCodec == misc.originalCodec)
                {
                    Log::w(
                        self.TAG,
                        "audio specific config do not changed, discard it.",
                    );
                    return;
                }

                meta.audioSampleRate = misc.samplingRate;
                meta.channelCount = misc.channelCount;
                meta.codec = misc.codec;
                meta.originalCodec = misc.originalCodec;
                meta.config = misc.config;
                // The decode result of an aac sample is 1024 PCM samples
                meta.refSampleDuration = 1024 / meta.audioSampleRate * meta.timescale;

                if (self._isInitialMetadataDispatched()) {
                    // Non-initial metadata, force dispatch (or flush) parsed frames to remuxer
                    if (self._dispatch && (self._audioTrack.length || self._videoTrack.length)) {
                        self._onDataAvailable(self._audioTrack, self._videoTrack);
                    }
                } else {
                    self._audioInitialMetadataDispatched = true;
                }
                // then notify new metadata
                self._dispatch = false;
                self._onTrackMetadata("audio", meta);

                let mi = self._mediaInfo;
                mi.audioCodec = meta.originalCodec;
                mi.audioSampleRate = meta.audioSampleRate;
                mi.audioChannelCount = meta.channelCount;

                //codecs format see https://tools.ietf.org/html/rfc6381
                if (mi.hasVideo) {
                    if (mi.videoCodec != null) {
                        //  mi.mimeType = "video/x-flv; codecs="" + mi.videoCodec + "," + mi.audioCodec + """;
                    }
                } else {
                    //  mi.mimeType = "video/x-flv; codecs="" + mi.audioCodec + """;
                }
                if (mi.isComplete()) {
                    self._onMediaInfo(mi);
                }
            } else if let AudioData::Data(data) = aacData {
                // AAC raw frame data
                let dts = self._timestampBase + tagTimestamp;
                let aacSample = Sample {
                    unit: data,
                    length: data.len(),
                    dts,
                    pts: dts,
                    units: (),
                    isKeyframe: (),
                    cts: (),
                };
                track.samples.push(aacSample);
                track.length += data.len();
            } else {
                Log::e(
                    self.TAG,
                    "Flv: Unsupported AAC data type ${aacData.packetType}",
                );
            }
        } else if soundFormat == 2 {
            // MP3
            if (!meta.codec) {
                // We need metadata for mp3 audio track, extract info from frame header
                let misc = self._parseMP3AudioData(arrayBuffer, dataOffset + 1, dataSize - 1, true);

                let misc = match misc {
                    Some(AudioData::Config(c)) => c,
                    _ => return,
                };

                meta.audioSampleRate = misc.samplingRate;
                meta.channelCount = misc.channelCount;
                meta.codec = misc.codec;
                meta.originalCodec = misc.originalCodec;
                // The decode result of an mp3 sample is 1152 PCM samples
                meta.refSampleDuration = 1152 / meta.audioSampleRate * meta.timescale;
                Log::v(self.TAG, "Parsed MPEG Audio Frame Header");

                self._audioInitialMetadataDispatched = true;
                self._onTrackMetadata("audio", meta);

                let mi = self._mediaInfo;
                mi.audioCodec = meta.codec;
                mi.audioSampleRate = meta.audioSampleRate;
                mi.audioChannelCount = meta.channelCount;
                mi.audioDataRate = misc.bitRate;
                if (mi.hasVideo) {
                    if (mi.videoCodec != null) {
                        //  mi.mimeType = "video/x-flv; codecs="" + mi.videoCodec + "," + mi.audioCodec + """;
                    }
                } else {
                    //  mi.mimeType = "video/x-flv; codecs="" + mi.audioCodec + """;
                }
                if (mi.isComplete()) {
                    self._onMediaInfo(mi);
                }
            }

            // This packet is always a valid audio packet, extract it
            let data = self._parseMP3AudioData(arrayBuffer, dataOffset + 1, dataSize - 1, false);
            if (data == undefined) {
                return;
            }
            let dts = self._timestampBase + tagTimestamp;
            //  let mp3Sample = {unit: data, length: data.byteLength, dts: dts, pts: dts};
            track.samples.push(mp3Sample);
            track.length += data.length;
        }
    }

    fn _parseAACAudioData(
        &self,
        arrayBuffer: &[u8],
        dataOffset: usize,
        dataSize: usize,
    ) -> Option<AudioData> {
        if dataSize <= 1 {
            Log::w(
                self.TAG,
                "Flv: Invalid AAC packet, missing AACPacketType or/and Data!",
            );
            return None;
        }

        let array = Cursor::new(&arrayBuffer[dataOffset..]);

        let result = if array[0] == 0 {
            AudioData::Config(self._parseAACAudioSpecificConfig(
                arrayBuffer,
                dataOffset + 1,
                dataSize - 1,
            ))
        } else {
            AudioData::Data(Vec::from(
                &arrayBuffer[dataOffset + 1..dataOffset + dataSize],
            ))
        };

        return Some(result);
    }

    fn _parseAACAudioSpecificConfig(
        &self,
        arrayBuffer: &[u8],
        dataOffset: usize,
        dataSize: usize,
    ) -> AudioConfig {
        let array = Cursor::new(arrayBuffer);
        let config;

        /* Audio Object Type:
           0: Null
           1: AAC Main
           2: AAC LC
           3: AAC SSR (Scalable Sample Rate)
           4: AAC LTP (Long Term Prediction)
           5: HE-AAC / SBR (Spectral Band Replication)
           6: AAC Scalable
        */

        let audioObjectType = 0;
        let originalAudioObjectType = 0;
        let audioExtensionObjectType;
        let samplingIndex = 0;
        let extensionSamplingIndex;

        // 5 bits
        audioObjectType = originalAudioObjectType = array[0] >> 3;
        // 4 bits
        samplingIndex = ((array[0] & 0x07) << 1) | (array[1] >> 7);
        if samplingIndex < 0 || samplingIndex >= self._mpegSamplingRates.length {
            self._onError.unwrap().call(
                &JsValue::null(),
                &JsValue::from(demux_errors::FORMAT_ERROR),
                &JsValue::from("Flv: AAC invalid sampling frequency index!"),
            );
            return;
        }

        let samplingFrequence = mpegSamplingRates[samplingIndex];

        // 4 bits
        let channelConfig = (array[1] & 0x78) >> 3;
        if channelConfig < 0 || channelConfig >= 8 {
            self._onError(
                demux_errors::FORMAT_ERROR,
                "Flv: AAC invalid channel configuration",
            );
            return;
        }

        if audioObjectType == 5 {
            // HE-AAC?
            // 4 bits
            extensionSamplingIndex = ((array[1] & 0x07) << 1) | (array[2] >> 7);
            // 5 bits
            audioExtensionObjectType = (array[2] & 0x7C) >> 2;
        }

        // workarounds for various browsers
        let userAgent: String = window()?.navigator()?.user_agent()?.to_lowercase();

        if (userAgent.find("firefox").is_some()) {
            // firefox: use SBR (HE-AAC) if freq less than 24kHz
            if (samplingIndex >= 6) {
                audioObjectType = 5;
                config = [0; 4];
                extensionSamplingIndex = samplingIndex - 3;
            } else {
                // use LC-AAC
                audioObjectType = 2;
                config = [0; 2];
                extensionSamplingIndex = samplingIndex;
            }
        } else if (userAgent.find("android").is_some()) {
            // android: always use LC-AAC
            audioObjectType = 2;
            config = [0; 2];
            extensionSamplingIndex = samplingIndex;
        } else {
            // for other browsers, e.g. chrome...
            // Always use HE-AAC to make it easier to switch aac codec profile
            audioObjectType = 5;
            extensionSamplingIndex = samplingIndex;
            config = [0; 4];

            if samplingIndex >= 6 {
                extensionSamplingIndex = samplingIndex - 3;
            } else if channelConfig == 1 {
                // Mono channel
                audioObjectType = 2;
                config = [0; 4];
                extensionSamplingIndex = samplingIndex;
            }
        }

        config[0] = audioObjectType << 3;
        config[0] |= (samplingIndex & 0x0F) >> 1;
        config[1] = (samplingIndex & 0x0F) << 7;
        config[1] |= (channelConfig & 0x0F) << 3;
        if audioObjectType == 5 {
            config[1] |= (extensionSamplingIndex & 0x0F) >> 1;
            config[2] = (extensionSamplingIndex & 0x01) << 7;
            // extended audio object type: force to 2 (LC-AAC)
            config[2] |= 2 << 2;
            config[3] = 0;
        }

        return AudioConfig {
            config: config,
            bitRate: 0,
            samplingRate: samplingFrequence,
            channelCount: channelConfig,
            codec: "mp4a.40." + audioObjectType,
            originalCodec: "mp4a.40." + originalAudioObjectType,
        };
    }

    fn _parseMP3AudioData(
        &self,
        arrayBuffer: &[u8],
        dataOffset: usize,
        dataSize: usize,
        requestHeader: bool,
    ) -> Option<AudioData> {
        if dataSize < 4 {
            Log::w(self.TAG, "Flv: Invalid MP3 packet, header missing!");
            return None;
        }

        let array = Cursor::new(&arrayBuffer[dataOffset..]);
        let result;

        if requestHeader {
            if array[0] != 0xFF {
                return;
            }
            let ver = (array[1] >> 3) & 0x03;
            let layer = (array[1] & 0x06) >> 1;

            let bitrate_index = (array[2] & 0xF0) >> 4;
            let sampling_freq_index = (array[2] & 0x0C) >> 2;

            let channel_mode = (array[3] >> 6) & 0x03;
            let channel_count = if channel_mode != 3 { 2 } else { 1 };

            let sample_rate = 0;
            let bit_rate = 0;
            let object_type = 34; // Layer-3, listed in MPEG-4 Audio Object Types

            let codec = "mp3";

            match ver {
                // MPEG 2.5
                0 => sample_rate = self._mpegAudioV25SampleRateTable[sampling_freq_index],
                // MPEG 2
                2 => sample_rate = self._mpegAudioV20SampleRateTable[sampling_freq_index],
                // MPEG 1
                3 => sample_rate = self._mpegAudioV10SampleRateTable[sampling_freq_index],
            }

            match layer {
                // Layer 3
                1 => {
                    object_type = 34;
                    if bitrate_index < self._mpegAudioL3BitRateTable.length {
                        bit_rate = self._mpegAudioL3BitRateTable[bitrate_index];
                    }
                }
                // Layer 2
                2 => {
                    object_type = 33;
                    if bitrate_index < self._mpegAudioL2BitRateTable.length {
                        bit_rate = self._mpegAudioL2BitRateTable[bitrate_index];
                    }
                }
                // Layer 1
                3 => {
                    object_type = 32;
                    if bitrate_index < self._mpegAudioL1BitRateTable.length {
                        bit_rate = self._mpegAudioL1BitRateTable[bitrate_index];
                    }
                }
            }

            result = AudioConfig {
                bitRate: bit_rate,
                samplingRate: sample_rate,
                channelCount: channel_count,
                codec: codec,
                originalCodec: codec,
            };
        } else {
            result = array;
        }

        return Some(result);
    }

    fn _parseVideoData(
        &self,
        arrayBuffer: &[u8],
        dataOffset: usize,
        dataSize: usize,
        tagTimestamp: u32,
        tagPosition: u32,
    ) {
        if dataSize <= 1 {
            Log::w(
                self.TAG,
                "Flv: Invalid video packet, missing VideoData payload!",
            );
            return;
        }

        if self._hasVideoFlagOverrided == true && self._hasVideo == false {
            // If hasVideo: false indicated explicitly in MediaDataSource,
            // Ignore all the video packets
            return;
        }

        let spec = arrayBuffer[0];

        let frameType = (spec & 0xF0) >> 4;
        let codecId = spec & 0x0F;

        if codecId != 7 {
            self._onError(
                demux_errors::CODEC_UNSUPPORTED,
                "Flv: Unsupported codec in video frame: ${codecId}",
            );
            return;
        }

        self._parseAVCVideoPacket(arrayBuffer, tagTimestamp, tagPosition, frameType);
    }

    fn _parseAVCVideoPacket(
        &self,
        arrayBuffer: &[u8],
        dataOffset: usize,
        dataSize: usize,
        tagTimestamp: u32,
        tagPosition: u32,
        frameType: u32,
    ) {
        if dataSize < 4 {
            Log::w(
                self.TAG,
                "Flv: Invalid AVC packet, missing AVCPacketType or/and CompositionTime",
            );
            return;
        }

        let v = Cursor::new(arrayBuffer);

        let packetType = v.read_u8()?;
        let cts = v.read_i24()?;
        // let cts = (cts_unsigned << 8) >> 8; // convert to 24-bit signed int

        if packetType == 0 {
            // AVCDecoderConfigurationRecord
            self._parseAVCDecoderConfigurationRecord(arrayBuffer);
        } else if packetType == 1 {
            // One or more Nalus
            self._parseAVCVideoData(arrayBuffer[4..], tagTimestamp, tagPosition, frameType, cts);
        } else if packetType == 2 {
            // empty, AVC end of sequence
        } else {
            self._onError(
                demux_errors::FORMAT_ERROR,
                "Flv: Invalid video packet type ${packetType}",
            );
            return;
        }
    }

    fn _parseAVCDecoderConfigurationRecord(
        &self,
        arrayBuffer: &[u8],
        dataOffset: usize,
        dataSize: usize,
    ) {
        if dataSize < 7 {
            Log::w(
                self.TAG,
                "Flv: Invalid AVCDecoderConfigurationRecord, lack of data!",
            );
            return;
        }

        let meta = self._videoMetadata;
        let track = self._videoTrack;
        let v = Cursor::new(arrayBuffer);

        let newAVCDecoderConfig = false;
        if (meta.is_none()) {
            if self._hasVideo == false && self._hasVideoFlagOverrided == false {
                self._hasVideo = true;
                self._mediaInfo.hasVideo = true;
            }

            meta = self._videoMetadata = Some(TrackMetaData {
                _type: "video",
                id: track.id,
                timescale: self._timescale,
                duration: self._duration,
                codec: "",
                data: &[],
            });
        } else {
            if meta.avcc != "undefined" {
                newAVCDecoderConfig = true;
                Log::v(
                    self.TAG,
                    "--== Found another AVCDecoderConfigurationRecord! ==-",
                );
            }
        }

        let version = v.read_u8(0)?; // configurationVersion
        let avcProfile = v.read_u8()?; // avcProfileIndication
        let profileCompatibility = v.read_u8()?; // profile_compatibility
        let avcLevel = v.read_u8()?; // AVCLevelIndication

        if version != 1 || avcProfile == 0 {
            self._onError(
                demux_errors::FORMAT_ERROR,
                "Flv: Invalid AVCDecoderConfigurationRecord",
            );
            return;
        }

        self._naluLengthSize = (v.read_u8()? & 3) + 1; // lengthSizeMinusOne
        if self._naluLengthSize != 3 && self._naluLengthSize != 4 {
            // holy shit!!!
            self._onError(
                demux_errors::FORMAT_ERROR,
                "Flv: Strange NaluLengthSizeMinusOne: ${self._naluLengthSize - 1}",
            );
            return;
        }

        let spsCount = v.read_u8()? & 0b00011111; // numOfSequenceParameterSets
        if spsCount == 0 {
            self._onError(
                demux_errors::FORMAT_ERROR,
                "Flv: Invalid AVCDecoderConfigurationRecord: No SPS",
            );
            return;
        } else if spsCount > 1 {
            Log::w(
                self.TAG,
                "Flv: Strange AVCDecoderConfigurationRecord: SPS Count = ${spsCount}",
            );
        }

        let isLandscapeView = true;
        let offset = 6;

        for i in 0..spsCount {
            let len = v.read_u16::<BigEndian>()?; // sequenceParameterSetLength
            offset += 2;

            if (len == 0) {
                continue;
            }

            // Notice: Nalu without startcode header (00 00 00 01)
            let sps = &arrayBuffer[v.position() as usize..];
            offset += len;

            let config = super::sps_parser::parseSPS(sps)?;
            if (i != 0) {
                // ignore other sps"s config
                continue;
            }

            //  Log::v(self.TAG, "Parsed AVCDecoderConfigurationRecord, sps:{codec_size: " + config.codec_size.width
            //      + "x" + config.codec_size.height + ", present_size: " +
            //      + config.present_size.width + "x" + config.present_size.height
            //      + ", profile: " + config.profile_string + ", level: " + config.level_string
            //      + ", fps: {" + config.frame_rate.fps_den + "," + config.frame_rate.fps_num
            //      + "," + config.frame_rate.fps + "," + config.frame_rate.fixed
            //      + "}, sar_ratio: " + config.sar_ratio.width
            //      + "x" + config.sar_ratio.height + "}");

            isLandscapeView = config.codec_size.width >= config.codec_size.height;

            let resolutionChanged = false;
            if (newAVCDecoderConfig && self._lastVideoDimension.is_some()) {
                // compare previous video width and height
                if (self._lastVideoDimension.width != config.codec_size.width
                    || self._lastVideoDimension.height != config.codec_size.height)
                {
                    // video width or height changed
                    resolutionChanged = true;

                    // save new video demension
                    self._lastVideoDimension.width = config.codec_size.width;
                    self._lastVideoDimension.height = config.codec_size.height;
                    self._lastVideoDimension.profile = config.profile_string;
                    self._lastVideoDimension.level = config.level_string;

                    // emit video_resolution_changed event
                    let video_info = {};
                    video_info.width = config.codec_size.width;
                    video_info.height = config.codec_size.height;
                    self._onVideoResolutionChanged(video_info);
                } else {
                    // width and height does not changed, compare profile and level
                    if self._lastVideoDimension.profile == config.profile_string
                        && self._lastVideoDimension.level == config.level_string
                    {
                        // Tecent Cloud would send AVCDecoderConfigurationRecord per second, I don"t know why!
                        Log::d(self.TAG, "video config does not changed. discard reset.");
                        return;
                    }
                }
            } else {
                // save the first video dimentsion
                self._lastVideoDimension.width = config.codec_size.width;
                self._lastVideoDimension.height = config.codec_size.height;
                self._lastVideoDimension.profile = config.profile_string;
                self._lastVideoDimension.level = config.level_string;
            }

            meta.codecWidth = config.codec_size.width;
            meta.codecHeight = config.codec_size.height;
            meta.presentWidth = config.present_size.width;
            meta.presentHeight = config.present_size.height;

            meta.profile = config.profile_string;
            meta.level = config.level_string;
            meta.bitDepth = config.bit_depth;
            meta.chromaFormat = config.chroma_format;
            meta.sarRatio = config.sar_ratio;
            meta.frameRate = config.frame_rate;

            if config.frame_rate.fixed == false
                || config.frame_rate.fps_num == 0
                || config.frame_rate.fps_den == 0
            {
                meta.frameRate = self._referenceFrameRate;
            }

            let fps_den = meta.frameRate.fps_den;
            let fps_num = meta.frameRate.fps_num;
            meta.refSampleDuration = meta.timescale * (fps_den / fps_num);

            let codecArray = sps[1..4];
            let codecString = format!(
                "avc1.{:x}{:x}{:x}",
                codecArray[0], codecArray[1], codecArray[2]
            );
            meta.codec = codecString;

            let mi = self._mediaInfo;
            mi.width = meta.codecWidth;
            mi.height = meta.codecHeight;
            mi.fps = meta.frameRate.fps;
            mi.profile = meta.profile;
            mi.level = meta.level;
            mi.refFrames = config.ref_frames;
            mi.chromaFormat = config.chroma_format_string;
            mi.sarNum = meta.sarRatio.width;
            mi.sarDen = meta.sarRatio.height;
            mi.videoCodec = codecString;

            if mi.hasAudio {
                if (mi.audioCodec != null) {
                    //  mi.mimeType = "video/x-flv; codecs="" + mi.videoCodec + "," + mi.audioCodec + """;
                }
            } else {
                //  mi.mimeType = "video/x-flv; codecs="" + mi.videoCodec + """;
            }
            if mi.isComplete() {
                //see transmuxing-controller.js::_onMediaInfo()
                self._onMediaInfo(mi);
            }
        }

        let ppsCount = v.read_u8()?; // numOfPictureParameterSets
        if ppsCount == 0 {
            self._onError(
                demux_errors::FORMAT_ERROR,
                "Flv: Invalid AVCDecoderConfigurationRecord: No PPS",
            );
            return;
        } else if ppsCount > 1 {
            Log::w(
                self.TAG,
                "Flv: Strange AVCDecoderConfigurationRecord: PPS Count = ${ppsCount}",
            );
        }

        offset += 1;

        for i in 0..ppsCount {
            let len = v.read_u16::<BigEndian>()?; // pictureParameterSetLength
            offset += 2;

            if len == 0 {
                continue;
            }

            // pps is useless for extracting video information
            offset += len;
        }

        meta.avcc = Cursor::new(dataSize);
        //  meta.avcc.set(new Uint8Array(arrayBuffer, dataOffset, dataSize), 0);

        Log::v(
            self.TAG,
            "Parsed AVCDecoderConfigurationRecord done, "
                + meta.codecWidth
                + "x"
                + meta.codecHeight
                + "@"
                + meta.frameRate.fps
                + " fps, "
                + "profile="
                + meta.profile
                + ", level="
                + meta.level,
        );

        if self._isInitialMetadataDispatched() {
            // flush parsed frames
            if self._dispatch && (self._audioTrack.length || self._videoTrack.length) {
                self._onDataAvailable(self._audioTrack, self._videoTrack);
            }
        } else {
            self._videoInitialMetadataDispatched = true;
        }
        // notify new metadata
        self._dispatch = false;

        if self._config.enableConstVideoViewSize {
            Log::w(
                self.TAG,
                "--== const video view size enabled, use {"
                    + self._config.constVideoViewWidth
                    + "x"
                    + self._config.constVideoViewHeight
                    + "} ==--",
            );
            meta.codecWidth = if isLandscapeView {
                self._config.constVideoViewWidth
            } else {
                self._config.constVideoViewHeight
            };
            meta.codecHeight = if isLandscapeView {
                self._config.constVideoViewHeight
            } else {
                self._config.constVideoViewWidth
            };
        }

        //see mp4-remuxer.js::_onTrackMetadataReceived()
        self._onTrackMetadata("video", meta);
    }

    fn _parseAVCVideoData(
        &self,
        arrayBuffer: &[u8],
        dataOffset: usize,
        dataSize: usize,
        tagTimestamp: u32,
        tagPosition: u32,
        frameType: u32,
        cts: i32,
    ) {
        let v = Cursor::new(arrayBuffer);

        let units = Vec::new();
        let length = 0;

        let offset = 0;
        let lengthSize = self._naluLengthSize;
        let dts = self._timestampBase + tagTimestamp;
        let keyframe = frameType == 1; // from FLV Frame Type constants

        while offset < arrayBuffer.len() {
            if offset + 4 >= arrayBuffer.len() {
                Log::w(self.TAG, "Malformed Nalu near timestamp ${dts}, offset = ${offset}, dataSize = ${dataSize}");
                break; // data not enough for next Nalu
            }
            // Nalu with length-header (AVC1)
            let naluSize = v.read_u32::<BigEndian>()?; // Big-Endian read
            if lengthSize == 3 {
                naluSize >>= 8;
            }
            if naluSize > arrayBuffer.len() - lengthSize {
                Log::w(
                    self.TAG,
                    "Malformed Nalus near timestamp ${dts}, NaluSize > DataSize!",
                );
                return;
            }

            let unitType = v.read_u8() & 0x1F;

            if unitType == 5 {
                // IDR
                keyframe = true;
            }

            let data: Vec<u8> = Vec::with_capacity(lengthSize + naluSize);
            v.read_exact(&data)?;
            let unit = Unit {
                _type: unitType,
                data: data,
            };
            units.push(unit);
            length += data.len();

            offset += lengthSize + naluSize;
        }

        if units.len() > 0 {
            let track = self._videoTrack;
            let avcSample = Sample {
                units,
                length,
                isKeyframe: keyframe,
                dts,
                cts,
                pts: (dts + cts),
                fileposition: None,
            };
            if keyframe {
                avcSample.fileposition = Some(tagPosition);
            }
            track.samples.push(avcSample);
            track.length += length;
        }
    }
}
