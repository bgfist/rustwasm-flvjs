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

//  import Log from "../utils/logger.js";
//  import MP4 from "./mp4-generator.js";
//  import AAC from "./aac-silent.js";
//  import Browser from "../utils/browser.js";
//  import { SampleInfo, MediaSegmentInfo, MediaSegmentInfoList } from "../core/media-segment-info.js";
//  import { IllegalStateException } from "../utils/exception.js";
use crate::core::media_segment_info::MediaSegmentInfo;
use crate::core::media_segment_info::MediaSegmentInfoList;
use crate::core::media_segment_info::SampleInfo;
use crate::demux::flv_demuxer::Track;
use crate::demux::flv_demuxer::{AudioTrack, FLVDemuxer, VideoTrack};
use crate::utils::logger::Log;
use js_sys::Function;
use js_sys::Math;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
struct InitSegment {
    pub r#type: &'static str,
    pub data: Vec<u8>,
    pub codec: String,
    pub container: String,
    pub mediaDuration: u32,
}

#[wasm_bindgen]
struct MediaSegment {
    pub r#type: &'static str,
    pub data: Vec<u8>,
    pub sampleCount: usize,
    pub info: MediaSegmentInfo,
}

pub struct MP4Remuxer {
    _dtsBase: i64,
    _audioDtsBase: i64,
    _videoDtsBase: i64,
    _audioNextDts: i64,
    _videoNextDts: i64,

    _onInitSegment: Option<Function>,
    _onMediaSegment: Option<Function>,

    _audioSegmentInfoList: MediaSegmentInfoList,
    _videoSegmentInfoList: MediaSegmentInfoList,
}

// Fragmented mp4 remuxer
impl MP4Remuxer {
    fn new(config: !) -> MP4Remuxer {
        //  self.TAG = "MP4Remuxer";

        self._config = config;
        self._isLive = (config.isLive == true);

        MP4Remuxer {
            _dtsBase: -1,
            _dtsBaseInited: false,
            _audioDtsBase: -1,
            _videoDtsBase: -1,
            _audioNextDts: -1,
            _videoNextDts: -1,
            _audioStashedLastSample: None,
            _videoStashedLastSample: None,
            _audioMeta: None,
            _videoMeta: None,
            _audioSegmentInfoList: Vec::new(),
            _videoSegmentInfoList: Vec::new(),
            _dtsBaseInited: false,
            _audioDtsBase: None,
            _videoDtsBase: None,
            _audioNextDts: None,
            _videoNextDts: None,
            _audioStashedLastSample: None,
            _videoStashedLastSample: None,

            _audioMeta: None,
            _videoMeta: None,

            _audioSegmentInfoList: MediaSegmentInfoList::new("audio"),
            _videoSegmentInfoList: MediaSegmentInfoList::new("video"),

            _onInitSegment: None,
            _onMediaSegment: None,
            // Workaround for chrome < 50: Always force first sample as a Random Access Point in media segment
            // see https://bugs.chromium.org/p/chromium/issues/detail?id:229412
            //  _forceFirstIDR : (Browser.chrome &&
            //      (Browser.version.major < 50 ||
            //          (Browser.version.major :: 50 && Browser.version.build < 2661))) ? true : false,

            // Workaround for IE11/Edge: Fill silent aac frame after keyframe-seeking
            // Make audio beginDts equals with video beginDts, in order to fix seek freeze
            //  _fillSilentAfterSeek : (Browser.msedge || Browser.msie),

            // While only FireFox supports "audio/mp4, codecs:"mp3"", use "audio/mpeg" for chrome, safari, ...
            //  _mp3UseMpegAudio : !Browser.firefox,

            //  _fillAudioTimestampGap : _config.fixAudioTimestampGap,
        }
    }

    fn destroy(&self) {
        self._dtsBase = -1;
        self._dtsBaseInited = false;
        self._audioMeta = None;
        self._videoMeta = None;
        self._audioSegmentInfoList.clear();
        self._audioSegmentInfoList = None;
        self._videoSegmentInfoList.clear();
        self._videoSegmentInfoList = None;
        self._onInitSegment = None;
        self._onMediaSegment = None;
    }

    fn bindDataSource(&self, producer: &FLVDemuxer) -> &Self {
        producer.set_onDataAvailable(MP4Remuxer::remux);
        producer.set_onTrackMetadata(MP4Remuxer::_onTrackMetadataReceived);
        return self;
    }

    #[wasm_bindgen(getter, js_name = "onInitSegment")]
    pub fn get_onInitSegment() -> Option<Function> {
        return self._onInitSegment;
    }

    #[wasm_bindgen(setter, js_name = "onInitSegment")]
    pub fn set_onInitSegment(callback: Option<Function>) {
        self._onInitSegment = callback;
    }

    #[wasm_bindgen(getter, js_name = "onMediaSegment")]
    pub fn get_onMediaSegment() -> Option<Function> {
        return self._onMediaSegment;
    }

    #[wasm_bindgen(getter, js_name = "onMediaSegment")]
    pub fn set_onMediaSegment(callback: Option<Function>) {
        self._onMediaSegment = callback;
    }

    fn insertDiscontinuity(&self) {
        self._audioNextDts = self._videoNextDts = undefined;
    }

    fn seek(&self, originalDts: u32) {
        self._audioStashedLastSample = None;
        self._videoStashedLastSample = None;
        self._videoSegmentInfoList.clear();
        self._audioSegmentInfoList.clear();
    }

    fn remux(&self, audioTrack: &mut AudioTrack, videoTrack: &mut VideoTrack) {
        if (!self._onMediaSegment) {
            //  throw new IllegalStateException("MP4Remuxer: onMediaSegment callback must be specificed!");
        }
        if (!self._dtsBaseInited) {
            self._calculateDtsBase(audioTrack, videoTrack);
        }
        self._remuxVideo(videoTrack);
        self._remuxAudio(audioTrack);
    }

    fn _onTrackMetadataReceived(&self, metadata: TrackMetaData) {
        let metabox = None;

        let container = "mp4";
        let codec = metadata.codec;

        //  if (_type == "audio") {
        //      self._audioMeta = metadata;
        //      if (metadata.codec == "mp3" && self._mp3UseMpegAudio) {
        //          // "audio/mpeg" for MP3 audio track
        //          container = "mpeg";
        //          codec = "";
        //          metabox = Vec::new();
        //      } else {
        //          // "audio/mp4, codecs="codec""
        //          metabox = MP4.generateInitSegment(metadata);
        //      }
        //  } else if (_type == "video") {
        //      self._videoMeta = metadata;
        //      metabox = MP4.generateInitSegment(metadata);
        //  } else {
        //      return;
        //  }

        match self._onInitSegment {
            Some(_onInitSegment) => _onInitSegment.call2(
                r#type,
                InitSegment {
                    r#type,
                    data: metabox.buffer,
                    codec: codec,
                    container: format!("{}/{}", r#type, container),
                    mediaDuration: metadata.duration, // in timescale 1000 (milliseconds)
                },
            ),
            None => panic("MP4Remuxer: onInitSegment callback must be specified!"),
        }
    }

    fn _calculateDtsBase(&self, audioTrack: &AudioTrack, videoTrack: &VideoTrack) {
        if (self._dtsBaseInited) {
            return;
        }

        if (audioTrack.samples && audioTrack.samples.length) {
            self._audioDtsBase = audioTrack.samples[0].dts;
        }
        if (videoTrack.samples && videoTrack.samples.length) {
            self._videoDtsBase = videoTrack.samples[0].dts;
        }

        self._dtsBase = Math.min(self._audioDtsBase, self._videoDtsBase);
        self._dtsBaseInited = true;
    }

    fn flushStashedSamples(&self) {
        let videoSample = self._videoStashedLastSample;
        let audioSample = self._audioStashedLastSample;

        let videoTrack = VideoTrack {
            r#type: "video",
            id: 1,
            sequenceNumber: 0,
            samples: [],
            length: 0,
        };

        if (videoSample != null) {
            videoTrack.samples.push(videoSample);
            videoTrack.length = videoSample.length;
        }

        let audioTrack = AudioTrack {
            r#type: "audio",
            id: 2,
            sequenceNumber: 0,
            samples: [],
            length: 0,
        };

        if (audioSample != null) {
            audioTrack.samples.push(audioSample);
            audioTrack.length = audioSample.length;
        }

        self._videoStashedLastSample = null;
        self._audioStashedLastSample = null;

        self._remuxVideo(videoTrack, true);
        self._remuxAudio(audioTrack, true);
    }

    fn _remuxAudio(&self, audioTrack: &mut AudioTrack, force: bool) {
        if (self._audioMeta == null) {
            return;
        }

        let track = audioTrack;
        let samples = track.samples;
        let dtsCorrection = undefined;
        let firstDts = -1;
        let lastDts = -1;
        let lastPts = -1;
        let refSampleDuration = self._audioMeta.refSampleDuration;

        let mpegRawTrack = self._audioMeta.codec == "mp3" && self._mp3UseMpegAudio;
        let firstSegmentAfterSeek = self._dtsBaseInited && self._audioNextDts == undefined;

        let insertPrefixSilentFrame = false;

        if (!samples || samples.length == 0) {
            return;
        }
        if (samples.length == 1 && !force) {
            // If [sample count in current batch] == 1 && (force != true)
            // Ignore and keep in demuxer"s queue
            return;
        } // else if (force == true) do remux

        let offset = 0;
        let mdatbox = null;
        let mdatBytes = 0;

        // calculate initial mdat size
        if (mpegRawTrack) {
            // for raw mpeg buffer
            offset = 0;
            mdatBytes = track.length;
        } else {
            // for fmp4 mdat box
            offset = 8; // size + type
            mdatBytes = 8 + track.length;
        }

        let lastSample = null;

        // Pop the lastSample and waiting for stash
        if (samples.length > 1) {
            lastSample = samples.pop();
            mdatBytes -= lastSample.length;
        }

        // Insert [stashed lastSample in the previous batch] to the front
        if (self._audioStashedLastSample != null) {
            let sample = self._audioStashedLastSample;
            self._audioStashedLastSample = null;
            samples.unshift(sample);
            mdatBytes += sample.length;
        }

        // Stash the lastSample of current batch, waiting for next batch
        if (lastSample != null) {
            self._audioStashedLastSample = lastSample;
        }

        let firstSampleOriginalDts = samples[0].dts - self._dtsBase;

        // calculate dtsCorrection
        if (self._audioNextDts) {
            dtsCorrection = firstSampleOriginalDts - self._audioNextDts;
        } else {
            // self._audioNextDts == undefined
            if (self._audioSegmentInfoList.isEmpty()) {
                dtsCorrection = 0;
                if (self._fillSilentAfterSeek && !self._videoSegmentInfoList.isEmpty()) {
                    if (self._audioMeta.originalCodec != "mp3") {
                        insertPrefixSilentFrame = true;
                    }
                }
            } else {
                let lastSample = self
                    ._audioSegmentInfoList
                    .getLastSampleBefore(firstSampleOriginalDts);
                if (lastSample != null) {
                    let distance =
                        (firstSampleOriginalDts - (lastSample.originalDts + lastSample.duration));
                    if (distance <= 3) {
                        distance = 0;
                    }
                    let expectedDts = lastSample.dts + lastSample.duration + distance;
                    dtsCorrection = firstSampleOriginalDts - expectedDts;
                } else {
                    // lastSample == null, cannot found
                    dtsCorrection = 0;
                }
            }
        }

        if (insertPrefixSilentFrame) {
            // align audio segment beginDts to match with current video segment"s beginDts
            let firstSampleDts = firstSampleOriginalDts - dtsCorrection;
            let videoSegment = self
                ._videoSegmentInfoList
                .getLastSegmentBefore(firstSampleOriginalDts);
            if (videoSegment != null && videoSegment.beginDts < firstSampleDts) {
                let silentUnit =
                    getSilentFrame(self._audioMeta.originalCodec, self._audioMeta.channelCount);
                if (silentUnit) {
                    let dts = videoSegment.beginDts;
                    let silentFrameDuration = firstSampleDts - videoSegment.beginDts;
                    Log::v(
                        self.TAG,
                        format!(
                            "InsertPrefixSilentAudio: dts: {}, duration: {silentFrameDuration}",
                            dts, silentFrameDuration
                        ),
                    );
                    samples.unshift(Unit {
                        unit: silentUnit,
                        dts: dts,
                        pts: dts,
                    });
                    mdatBytes += silentUnit.byteLength;
                } // silentUnit == null: Cannot generate, skip
            } else {
                insertPrefixSilentFrame = false;
            }
        }

        let mp4Samples = vec![];

        // Correct dts for each sample, and calculate sample duration. Then output to mp4Samples
        for sample in samples {
            let unit = sample.unit;
            let originalDts = sample.dts - self._dtsBase;
            let dts = originalDts;
            let needFillSilentFrames = false;
            let silentFrames = null;
            let sampleDuration = 0;

            if (originalDts < -0.001) {
                continue; //pass the first sample with the invalid dts
            }

            if (self._audioMeta.codec != "mp3") {
                // for AAC codec, we need to keep dts increase based on refSampleDuration
                let curRefDts = originalDts;
                const maxAudioFramesDrift: u32 = 3;
                if (self._audioNextDts) {
                    curRefDts = self._audioNextDts;
                }

                dtsCorrection = originalDts - curRefDts;
                if (dtsCorrection <= -maxAudioFramesDrift * refSampleDuration) {
                    // If we"re overlapping by more than maxAudioFramesDrift number of frame, drop this sample
                    Log::w(self.TAG, format!("Dropping 1 audio frame (originalDts: {} ms ,curRefDts: {} ms)  due to dtsCorrection: {} ms overlap.", originalDts, curRefDts, dtsCorrection));
                    continue;
                } else if (dtsCorrection >= maxAudioFramesDrift * refSampleDuration
                    && self._fillAudioTimestampGap
                    && !Browser.safari)
                {
                    // Silent frame generation, if large timestamp gap detected && config.fixAudioTimestampGap
                    needFillSilentFrames = true;
                    // We need to insert silent frames to fill timestamp gap
                    let frameCount = Math.floor(dtsCorrection / refSampleDuration);

                    dts = Math.floor(curRefDts);
                    sampleDuration = Math.floor(curRefDts + refSampleDuration) - dts;

                    let silentUnit =
                        getSilentFrame(self._audioMeta.originalCodec, self._audioMeta.channelCount);
                    if (silentUnit == null) {
                        // Repeat last frame
                        silentUnit = unit;
                    }
                    silentFrames = [];

                    for j in 0..frameCount {
                        curRefDts = curRefDts + refSampleDuration;
                        let intDts = Math.floor(curRefDts); // change to integer
                        let intDuration = Math.floor(curRefDts + refSampleDuration) - intDts;
                        let frame = Frame {
                            dts: intDts,
                            pts: intDts,
                            cts: 0,
                            unit: silentUnit,
                            size: silentUnit.byteLength,
                            duration: intDuration, // wait for next sample
                            originalDts: originalDts,
                            flags: Flag {
                                isLeading: 0,
                                dependsOn: 1,
                                isDependedOn: 0,
                                hasRedundancy: 0,
                            },
                        };
                        silentFrames.push(frame);
                        mdatBytes += unit.byteLength;
                    }

                    self._audioNextDts = curRefDts + refSampleDuration;
                } else {
                    dts = Math.floor(curRefDts);
                    sampleDuration = Math.floor(curRefDts + refSampleDuration) - dts;
                    self._audioNextDts = curRefDts + refSampleDuration;
                }
            } else {
                // keep the original dts calculate algorithm for mp3
                dts = originalDts - dtsCorrection;

                if (i != samples.length - 1) {
                    let nextDts = samples[i + 1].dts - self._dtsBase - dtsCorrection;
                    sampleDuration = nextDts - dts;
                } else {
                    // the last sample
                    if (lastSample != null) {
                        // use stashed sample"s dts to calculate sample duration
                        let nextDts = lastSample.dts - self._dtsBase - dtsCorrection;
                        sampleDuration = nextDts - dts;
                    } else if (mp4Samples.length >= 1) {
                        // use second last sample duration
                        sampleDuration = mp4Samples[mp4Samples.length - 1].duration;
                    } else {
                        // the only one sample, use reference sample duration
                        sampleDuration = Math::floor(refSampleDuration);
                    }
                }
                self._audioNextDts = dts + sampleDuration;
            }

            if (firstDts == -1) {
                firstDts = dts;
            }
            mp4Samples.push(SampleInfo {
                dts: dts,
                pts: dts,
                cts: 0,
                unit: sample.unit,
                size: sample.unit.byteLength,
                duration: sampleDuration,
                originalDts: originalDts,
                flags: Flag {
                    isLeading: 0,
                    dependsOn: 1,
                    isDependedOn: 0,
                    hasRedundancy: 0,
                },
            });

            if (needFillSilentFrames) {
                // Silent frames should be inserted after wrong-duration frame
                mp4Samples.push.apply(mp4Samples, silentFrames);
            }
        }

        if (mp4Samples.length == 0) {
            //no samples need to remux
            track.samples = [];
            track.length = 0;
            return;
        }

        // allocate mdatbox
        if (mpegRawTrack) {
            // allocate for raw mpeg buffer
            mdatbox = Vec::with_capacity(mdatBytes);
        } else {
            // allocate for fmp4 mdat box
            mdatbox = Vec::with_capacity(mdatBytes);
            // size field
            mdatbox[0] = (mdatBytes >> 24) & 0xFF;
            mdatbox[1] = (mdatBytes >> 16) & 0xFF;
            mdatbox[2] = (mdatBytes >> 8) & 0xFF;
            mdatbox[3] = (mdatBytes) & 0xFF;
            // type field (fourCC)
            mdatbox.set(MP4.types.mdat, 4);
        }

        // Write samples into mdatbox
        for i in 0..mp4Samples.length {
            let unit = mp4Samples[i].unit;
            mdatbox.set(unit, offset);
            offset += unit.byteLength;
        }

        let latest = mp4Samples[mp4Samples.length - 1];
        lastDts = latest.dts + latest.duration;
        //self._audioNextDts = lastDts;

        // fill media segment info & add to info list
        let info = MediaSegmentInfo::new();
        info.beginDts = firstDts;
        info.endDts = lastDts;
        info.beginPts = firstDts;
        info.endPts = lastDts;
        info.originalBeginDts = mp4Samples[0].originalDts;
        info.originalEndDts = latest.originalDts + latest.duration;
        info.firstSample = SampleInfo::new(
            mp4Samples[0].dts,
            mp4Samples[0].pts,
            mp4Samples[0].duration,
            mp4Samples[0].originalDts,
            false,
        );
        info.lastSample = SampleInfo::new(
            latest.dts,
            latest.pts,
            latest.duration,
            latest.originalDts,
            false,
        );
        if (!self._isLive) {
            self._audioSegmentInfoList.append(info);
        }

        track.samples = mp4Samples;
        track.sequenceNumber += 1;

        let moofbox = null;

        if (mpegRawTrack) {
            // Generate empty buffer, because useless for raw mpeg
            moofbox = Vec::new();
        } else {
            // Generate moof for fmp4 segment
            moofbox = MP4.moof(track, firstDts);
        }

        track.samples = [];
        track.length = 0;

        let segment = MediaSegment {
            r#type: "audio",
            data: self._mergeBoxes(moofbox, mdatbox).buffer,
            sampleCount: mp4Samples.length,
            info: info,
        };

        if (mpegRawTrack && firstSegmentAfterSeek) {
            // For MPEG audio stream in MSE, if seeking occurred, before appending new buffer
            // We need explicitly set timestampOffset to the desired point in timeline for mpeg SourceBuffer.
            segment.timestampOffset = firstDts;
        }

        self._onMediaSegment.call2(
            &JsValue::null(),
            &JsValue::from("audio"),
            &JsValue::from(segment),
        );
    }

    fn _remuxVideo(&self, videoTrack: &mut VideoTrack, force: bool) {
        if (self._videoMeta == null) {
            return;
        }

        let track = videoTrack;
        let samples = track.samples;
        let dtsCorrection = undefined;
        let firstDts = -1;
        let lastDts = -1;
        let firstPts = -1;
        let lastPts = -1;

        if (!samples || samples.length == 0) {
            return;
        }
        if (samples.length == 1 && !force) {
            // If [sample count in current batch] == 1 && (force != true)
            // Ignore and keep in demuxer"s queue
            return;
        } // else if (force == true) do remux

        let offset = 8;
        let mdatbox = null;
        let mdatBytes = 8 + videoTrack.length;

        let lastSample = null;

        // Pop the lastSample and waiting for stash
        if (samples.length > 1) {
            lastSample = samples.pop();
            mdatBytes -= lastSample.length;
        }

        // Insert [stashed lastSample in the previous batch] to the front
        if (self._videoStashedLastSample != null) {
            let sample = self._videoStashedLastSample;
            self._videoStashedLastSample = null;
            samples.unshift(sample);
            mdatBytes += sample.length;
        }

        // Stash the lastSample of current batch, waiting for next batch
        if (lastSample != null) {
            self._videoStashedLastSample = lastSample;
        }

        let firstSampleOriginalDts = samples[0].dts - self._dtsBase;

        // calculate dtsCorrection
        if (self._videoNextDts) {
            dtsCorrection = firstSampleOriginalDts - self._videoNextDts;
        } else {
            // self._videoNextDts == undefined
            if (self._videoSegmentInfoList.isEmpty()) {
                dtsCorrection = 0;
            } else {
                let lastSample = self
                    ._videoSegmentInfoList
                    .getLastSampleBefore(firstSampleOriginalDts);
                if (lastSample != null) {
                    let distance =
                        (firstSampleOriginalDts - (lastSample.originalDts + lastSample.duration));
                    if (distance <= 3) {
                        distance = 0;
                    }
                    let expectedDts = lastSample.dts + lastSample.duration + distance;
                    dtsCorrection = firstSampleOriginalDts - expectedDts;
                } else {
                    // lastSample == null, cannot found
                    dtsCorrection = 0;
                }
            }
        }

        let info = MediaSegmentInfo::new();
        let mp4Samples = [];

        // Correct dts for each sample, and calculate sample duration. Then output to mp4Samples
        for i in 0..samples.length {
            let sample = samples[i];
            let originalDts = sample.dts - self._dtsBase;
            let isKeyframe = sample.isKeyframe;
            let dts = originalDts - dtsCorrection;
            let cts = sample.cts;
            let pts = dts + cts;

            if (firstDts == -1) {
                firstDts = dts;
                firstPts = pts;
            }

            let sampleDuration = 0;

            if (i != samples.length - 1) {
                let nextDts = samples[i + 1].dts - self._dtsBase - dtsCorrection;
                sampleDuration = nextDts - dts;
            } else {
                // the last sample
                if (lastSample != null) {
                    // use stashed sample"s dts to calculate sample duration
                    let nextDts = lastSample.dts - self._dtsBase - dtsCorrection;
                    sampleDuration = nextDts - dts;
                } else if (mp4Samples.length >= 1) {
                    // use second last sample duration
                    sampleDuration = mp4Samples[mp4Samples.length - 1].duration;
                } else {
                    // the only one sample, use reference sample duration
                    sampleDuration = Math.floor(self._videoMeta.refSampleDuration);
                }
            }

            if (isKeyframe) {
                let syncPoint = SampleInfo::new(dts, pts, sampleDuration, sample.dts, true);
                syncPoint.fileposition = sample.fileposition;
                info.appendSyncPoint(syncPoint);
            }

            mp4Samples.push(SampleInfo {
                dts: dts,
                pts: pts,
                cts: cts,
                units: sample.units,
                size: sample.length,
                isKeyframe: isKeyframe,
                duration: sampleDuration,
                originalDts: originalDts,
                flags: Flag {
                    isLeading: 0,
                    dependsOn: if isKeyframe { 2 } else { 1 },
                    isDependedOn: if isKeyframe { 1 } else { 0 },
                    hasRedundancy: 0,
                    isNonSync: if isKeyframe { 0 } else { 1 },
                },
            });
        }

        // allocate mdatbox
        mdatbox = Vec::with_capacity(mdatBytes);
        mdatbox[0] = (mdatBytes >> 24) & 0xFF;
        mdatbox[1] = (mdatBytes >> 16) & 0xFF;
        mdatbox[2] = (mdatBytes >> 8) & 0xFF;
        mdatbox[3] = (mdatBytes) & 0xFF;
        mdatbox.set(MP4.types.mdat, 4);

        // Write samples into mdatbox
        for i in 0..mp4Samples.length {
            let units = mp4Samples[i].units;
            while (units.length) {
                let unit = units.shift();
                let data = unit.data;
                mdatbox.set(data, offset);
                offset += data.byteLength;
            }
        }

        let latest = mp4Samples[mp4Samples.length - 1];
        lastDts = latest.dts + latest.duration;
        lastPts = latest.pts + latest.duration;
        self._videoNextDts = lastDts;

        // fill media segment info & add to info list
        info.beginDts = firstDts;
        info.endDts = lastDts;
        info.beginPts = firstPts;
        info.endPts = lastPts;
        info.originalBeginDts = mp4Samples[0].originalDts;
        info.originalEndDts = latest.originalDts + latest.duration;
        info.firstSample = SampleInfo::new(
            mp4Samples[0].dts,
            mp4Samples[0].pts,
            mp4Samples[0].duration,
            mp4Samples[0].originalDts,
            mp4Samples[0].isKeyframe,
        );
        info.lastSample = SampleInfo::new(
            latest.dts,
            latest.pts,
            latest.duration,
            latest.originalDts,
            latest.isKeyframe,
        );
        if (!self._isLive) {
            self._videoSegmentInfoList.append(info);
        }

        track.samples = mp4Samples;
        track.sequenceNumber += 1;

        // workaround for chrome < 50: force first sample as a random access point
        // see https://bugs.chromium.org/p/chromium/issues/detail?id=229412
        if (self._forceFirstIDR) {
            let flags = mp4Samples[0].flags;
            flags.dependsOn = 2;
            flags.isNonSync = 0;
        }

        let moofbox = MP4.moof(track, firstDts);
        track.samples = [];
        track.length = 0;

        self._onMediaSegment(
            "video",
            MediaSegment {
                r#type: "video",
                data: self._mergeBoxes(moofbox, mdatbox).buffer,
                sampleCount: mp4Samples.length,
                info: info,
            },
        );
    }

    fn _mergeBoxes(&self, moof: Vec<u8>, mdat: Vec<u8>) -> Vec<u8> {
        //  let result = new Uint8Array(moof.byteLength + mdat.byteLength);
        //  result.set(moof, 0);
        //  result.set(mdat, moof.byteLength);
        //  return result;

        moof.extend_from_slice(mdat)
    }
}
