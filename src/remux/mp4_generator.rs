/*
 * Copyright (C) 2016 Bilibili. All Rights Reserved.
 *
 * This file is derived from dailymotion"s hls.js library (hls.js/src/remux/mp4-generator.js)
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

use crate::demux::flv_demuxer::{TrackMetaData, VideoTrack};

mod constants {
    pub const FTYP: [u8] = [
        0x69, 0x73, 0x6F, 0x6D, // major_brand: isom
        0x0, 0x0, 0x0, 0x1, // minor_version: 0x01
        0x69, 0x73, 0x6F, 0x6D, // isom
        0x61, 0x76, 0x63, 0x31, // avc1
    ];

    pub const STSD_PREFIX: [u8] = [
        0x00, 0x00, 0x00, 0x00, // version(0) + flags
        0x00, 0x00, 0x00, 0x01, // entry_count
    ];

    pub const STTS: [u8] = [
        0x00, 0x00, 0x00, 0x00, // version(0) + flags
        0x00, 0x00, 0x00, 0x00, // entry_count
    ];

    // pub const STSC = STCO; = STTS;

    pub const STSZ: [u8] = [
        0x00, 0x00, 0x00, 0x00, // version(0) + flags
        0x00, 0x00, 0x00, 0x00, // sample_size
        0x00, 0x00, 0x00, 0x00, // sample_count
    ];

    pub const HDLR_VIDEO: [u8] = [
        0x00, 0x00, 0x00, 0x00, // version(0) + flags
        0x00, 0x00, 0x00, 0x00, // pre_defined
        0x76, 0x69, 0x64, 0x65, // handler_type: "vide"
        0x00, 0x00, 0x00, 0x00, // reserved: 3 * 4 bytes
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x56, 0x69, 0x64, 0x65, 0x6F, 0x48, 0x61,
        0x6E, 0x64, 0x6C, 0x65, 0x72, 0x00, // name: VideoHandler
    ];

    pub const HDLR_AUDIO: [u8] = [
        0x00, 0x00, 0x00, 0x00, // version(0) + flags
        0x00, 0x00, 0x00, 0x00, // pre_defined
        0x73, 0x6F, 0x75, 0x6E, // handler_type: "soun"
        0x00, 0x00, 0x00, 0x00, // reserved: 3 * 4 bytes
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x53, 0x6F, 0x75, 0x6E, 0x64, 0x48, 0x61,
        0x6E, 0x64, 0x6C, 0x65, 0x72, 0x00, // name: SoundHandler
    ];

    pub const DREF: [u8] = [
        0x00, 0x00, 0x00, 0x00, // version(0) + flags
        0x00, 0x00, 0x00, 0x01, // entry_count
        0x00, 0x00, 0x00, 0x0C, // entry_size
        0x75, 0x72, 0x6C, 0x20, // type "url "
        0x00, 0x00, 0x00, 0x01, // version(0) + flags
    ];

    // Sound media header
    pub const SMHD: [u8] = [
        0x00, 0x00, 0x00, 0x00, // version(0) + flags
        0x00, 0x00, 0x00, 0x00, // balance(2) + reserved(2)
    ];

    // video media header
    pub const VMHD: [u8] = [
        0x00, 0x00, 0x00, 0x01, // version(0) + flags
        0x00, 0x00, // graphicsmode: 2 bytes
        0x00, 0x00, 0x00, 0x00, // opcolor: 3 * 2 bytes
        0x00, 0x00,
    ];
}

struct Meta {
    id: u32,
    _type: String,
}

// Generate a box
fn genBox(boxType: &[u8], datas: &[&[u8]]) -> Vec<u8> {
    let size = 8;
    let arrayCount = datas.len();

    for i in 0..arrayCount {
        size += datas[i].len();
    }

    let result = Vec::with_capacity(size);
    result[0] = (size >> 24) & 0xFF; // size
    result[1] = (size >> 16) & 0xFF;
    result[2] = (size >> 8) & 0xFF;
    result[3] = (size) & 0xFF;

    result.extend_from_slice(boxType); // type

    for i in 0..arrayCount {
        // data body
        result.extend_from_slice(datas[i]);
    }

    return result;
}

// emit ftyp & moov
fn generateInitSegment(meta: Meta) -> Vec<u8> {
    let ftyp = genBox(b"ftyp", &[constants::FTYP]);
    let moov = moov(meta);
    return ftyp.extend(moov);
}

// Movie metadata box
fn moov(meta: Meta) -> Vec<u8> {
    let mvhd = mvhd(meta.timescale, meta.duration);
    let trak = trak(meta);
    let mvex = mvex(meta);
    return genBox(b"moov", mvhd, trak, mvex);
}

// Movie header box
fn mvhd(timescale: u32, duration: u32) -> Vec<u8> {
    return genBox(
        b"mvhd",
        &[&[
            0x00,
            0x00,
            0x00,
            0x00, // version(0) + flags
            0x00,
            0x00,
            0x00,
            0x00, // creation_time
            0x00,
            0x00,
            0x00,
            0x00,                     // modification_time
            (timescale >> 24) & 0xFF, // timescale: 4 bytes
            (timescale >> 16) & 0xFF,
            (timescale >> 8) & 0xFF,
            (timescale) & 0xFF,
            (duration >> 24) & 0xFF, // duration: 4 bytes
            (duration >> 16) & 0xFF,
            (duration >> 8) & 0xFF,
            (duration) & 0xFF,
            0x00,
            0x01,
            0x00,
            0x00, // Preferred rate: 1.0
            0x01,
            0x00,
            0x00,
            0x00, // PreferredVolume(1.0, 2bytes) + reserved(2bytes)
            0x00,
            0x00,
            0x00,
            0x00, // reserved: 4 + 4 bytes
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x01,
            0x00,
            0x00, // ----begin composition matrix----
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x01,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x40,
            0x00,
            0x00,
            0x00, // ----end composition matrix----
            0x00,
            0x00,
            0x00,
            0x00, // ----begin pre_defined 6 * 4 bytes----
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00, // ----end pre_defined 6 * 4 bytes----
            0xFF,
            0xFF,
            0xFF,
            0xFF, // next_track_ID
        ]],
    );
}

// Track box
fn trak(meta: Meta) -> Vec<u8> {
    return genBox(b"trak", &[tkhd(meta), mdia(meta)]);
}

// Track header box
fn tkhd(meta: Meta) -> Vec<u8> {
    let trackId = meta.id;
    let duration = meta.duration;
    let width = meta.presentWidth;
    let height = meta.presentHeight;

    return genBox(
        b"tkhd",
        &[&[
            0x00,
            0x00,
            0x00,
            0x07, // version(0) + flags
            0x00,
            0x00,
            0x00,
            0x00, // creation_time
            0x00,
            0x00,
            0x00,
            0x00,                   // modification_time
            (trackId >> 24) & 0xFF, // track_ID: 4 bytes
            (trackId >> 16) & 0xFF,
            (trackId >> 8) & 0xFF,
            (trackId) & 0xFF,
            0x00,
            0x00,
            0x00,
            0x00,                    // reserved: 4 bytes
            (duration >> 24) & 0xFF, // duration: 4 bytes
            (duration >> 16) & 0xFF,
            (duration >> 8) & 0xFF,
            (duration) & 0xFF,
            0x00,
            0x00,
            0x00,
            0x00, // reserved: 2 * 4 bytes
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00, // layer(2bytes) + alternate_group(2bytes)
            0x00,
            0x00,
            0x00,
            0x00, // volume(2bytes) + reserved(2bytes)
            0x00,
            0x01,
            0x00,
            0x00, // ----begin composition matrix----
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x01,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x40,
            0x00,
            0x00,
            0x00,                // ----end composition matrix----
            (width >> 8) & 0xFF, // width and height
            (width) & 0xFF,
            0x00,
            0x00,
            (height >> 8) & 0xFF,
            (height) & 0xFF,
            0x00,
            0x00,
        ]],
    );
}

// Media Box
fn mdia(meta: Meta) -> Vec<u8> {
    return genBox(b"mdia", &[&mdhd(meta), &hdlr(meta), &minf(meta)]);
}

// Media header box
fn mdhd(meta: Meta) -> Vec<u8> {
    let timescale = meta.timescale;
    let duration = meta.duration;
    return genBox(
        b"mdhd",
        [
            0x00,
            0x00,
            0x00,
            0x00, // version(0) + flags
            0x00,
            0x00,
            0x00,
            0x00, // creation_time
            0x00,
            0x00,
            0x00,
            0x00,                     // modification_time
            (timescale >> 24) & 0xFF, // timescale: 4 bytes
            (timescale >> 16) & 0xFF,
            (timescale >> 8) & 0xFF,
            (timescale) & 0xFF,
            (duration >> 24) & 0xFF, // duration: 4 bytes
            (duration >> 16) & 0xFF,
            (duration >> 8) & 0xFF,
            (duration) & 0xFF,
            0x55,
            0xC4, // language: und (undetermined)
            0x00,
            0x00, // pre_defined = 0
        ],
    );
}

// Media handler reference box
fn hdlr(meta: Meta) -> Vec<u8> {
    let data;
    if (meta._type == "audio") {
        data = constants::HDLR_AUDIO;
    } else {
        data = constants::HDLR_VIDEO;
    }
    return genBox(b"hdlr", &[&data]);
}

// Media infomation box
fn minf(meta: Meta) -> Vec<u8> {
    let xmhd;
    if (meta._type == "audio") {
        xmhd = genBox(b"smhd", &[&constants::SMHD]);
    } else {
        xmhd = genBox(b"vmhd", &[&constants::VMHD]);
    }
    return genBox(b"minf", &[&xmhd, &dinf(), &stbl(meta)]);
}

// Data infomation box
fn dinf() -> Vec<u8> {
    let result = genBox(b"dinf", genBox(b"dref", &[&constants::DREF]));
    return result;
}

// Sample table box
fn stbl(meta: Meta) -> Vec<u8> {
    let result = genBox(
        b"stbl",                          // type: stbl
        stsd(meta),                       // Sample Description Table
        genBox(b"stts", constants::STTS), // Time-To-Sample
        genBox(b"stsc", constants::STTS), // Sample-To-Chunk
        genBox(b"stsz", constants::STSZ), // Sample size
        genBox(b"stco", constants::STTS), // Chunk offset
    );
    return result;
}

// Sample description box
fn stsd(meta: Meta) -> Vec<u8> {
    if (meta._type == "audio") {
        if (meta.codec == "mp3") {
            return genBox(b"stsd", constants::STSD_PREFIX, mp3(meta));
        }
        // else: aac -> mp4a
        return genBox(b"stsd", constants::STSD_PREFIX, mp4a(meta));
    } else {
        return genBox(b"stsd", constants::STSD_PREFIX, avc1(meta));
    }
}

fn mp3(meta: Meta) -> Vec<u8> {
    let channelCount = meta.channelCount;
    let sampleRate = meta.audioSampleRate;

    let data = [
        0x00,
        0x00,
        0x00,
        0x00, // reserved(4)
        0x00,
        0x00,
        0x00,
        0x01, // reserved(2) + data_reference_index(2)
        0x00,
        0x00,
        0x00,
        0x00, // reserved: 2 * 4 bytes
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        channelCount, // channelCount(2)
        0x00,
        0x10, // sampleSize(2)
        0x00,
        0x00,
        0x00,
        0x00,                     // reserved(4)
        (sampleRate >> 8) & 0xFF, // Audio sample rate
        (sampleRate) & 0xFF,
        0x00,
        0x00,
    ];

    return genBox(b".mp3", &[&data]);
}

fn mp4a(meta: Meta) -> Vec<u8> {
    let channelCount = meta.channelCount;
    let sampleRate = meta.audioSampleRate;

    let data = [
        0x00,
        0x00,
        0x00,
        0x00, // reserved(4)
        0x00,
        0x00,
        0x00,
        0x01, // reserved(2) + data_reference_index(2)
        0x00,
        0x00,
        0x00,
        0x00, // reserved: 2 * 4 bytes
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        channelCount, // channelCount(2)
        0x00,
        0x10, // sampleSize(2)
        0x00,
        0x00,
        0x00,
        0x00,                     // reserved(4)
        (sampleRate >> 8) & 0xFF, // Audio sample rate
        (sampleRate) & 0xFF,
        0x00,
        0x00,
    ];

    return genBox(b"mp4a", &[&data, &esds(meta)]);
}

fn esds(meta: Meta) -> Vec<u8> {
    let config = meta.config || [];
    let configSize = config.length;
    let data = [
        0x00,
        0x00,
        0x00,
        0x00,              // version 0 + flags
        0x03,              // descriptor_type
        0x17 + configSize, // length3
        0x00,
        0x01,              // es_id
        0x00,              // stream_priority
        0x04,              // descriptor_type
        0x0F + configSize, // length
        0x40,              // codec: mpeg4_audio
        0x15,              // stream_type: Audio
        0x00,
        0x00,
        0x00, // buffer_size
        0x00,
        0x00,
        0x00,
        0x00, // maxBitrate
        0x00,
        0x00,
        0x00,
        0x00, // avgBitrate
        0x05, // descriptor_type
    ];
    data.extend_from_slice(&[configSize]);
    data.extend_from_slice(config);
    data.extend_from_slice(&[
        0x06, 0x01, 0x02, // GASpecificConfig
    ]);

    return genBox(b"esds", &[&data]);
}

fn avc1(meta: Meta) -> Vec<u8> {
    let avcc = meta.avcc;
    let width = meta.codecWidth;
    let height = meta.codecHeight;

    let data = [
        0x00,
        0x00,
        0x00,
        0x00, // reserved(4)
        0x00,
        0x00,
        0x00,
        0x01, // reserved(2) + data_reference_index(2)
        0x00,
        0x00,
        0x00,
        0x00, // pre_defined(2) + reserved(2)
        0x00,
        0x00,
        0x00,
        0x00, // pre_defined: 3 * 4 bytes
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        (width >> 8) & 0xFF, // width: 2 bytes
        (width) & 0xFF,
        (height >> 8) & 0xFF, // height: 2 bytes
        (height) & 0xFF,
        0x00,
        0x48,
        0x00,
        0x00, // horizresolution: 4 bytes
        0x00,
        0x48,
        0x00,
        0x00, // vertresolution: 4 bytes
        0x00,
        0x00,
        0x00,
        0x00, // reserved: 4 bytes
        0x00,
        0x01, // frame_count
        0x0A, // strlen
        0x78,
        0x71,
        0x71,
        0x2F, // compressorname: 32 bytes
        0x66,
        0x6C,
        0x76,
        0x2E,
        0x6A,
        0x73,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x18, // depth
        0xFF,
        0xFF, // pre_defined = -1
    ];
    return genBox(b"avc1", &[&data, &genBox(b"avcC", avcc)]);
}

// Movie Extends box
fn mvex(meta: Meta) -> Vec<u8> {
    return genBox(b"mvex", &[&trex(meta)]);
}

// Track Extends box
fn trex(meta: Meta) -> Vec<u8> {
    let trackId = meta.id;
    let data = [
        0x00,
        0x00,
        0x00,
        0x00,                   // version(0) + flags
        (trackId >> 24) & 0xFF, // track_ID
        (trackId >> 16) & 0xFF,
        (trackId >> 8) & 0xFF,
        (trackId) & 0xFF,
        0x00,
        0x00,
        0x00,
        0x01, // default_sample_description_index
        0x00,
        0x00,
        0x00,
        0x00, // default_sample_duration
        0x00,
        0x00,
        0x00,
        0x00, // default_sample_size
        0x00,
        0x01,
        0x00,
        0x01, // default_sample_flags
    ];
    return genBox(b"trex", &[&data]);
}

// Movie fragment box
fn moof(track: &VideoTrack, baseMediaDecodeTime: u32) -> Vec<u8> {
    return genBox(
        b"moof",
        &[
            &mfhd(track.sequenceNumber),
            &traf(track, baseMediaDecodeTime),
        ],
    );
}

fn mfhd(sequenceNumber: u32) -> Vec<u8> {
    let data = vec![
        0x00,
        0x00,
        0x00,
        0x00,
        (sequenceNumber >> 24) & 0xFF, // sequence_number: int32
        (sequenceNumber >> 16) & 0xFF,
        (sequenceNumber >> 8) & 0xFF,
        (sequenceNumber) & 0xFF,
    ];
    return genBox(b"mfhd", data);
}

// Track fragment box
fn traf(track: &VideoTrack, baseMediaDecodeTime: u32) -> Vec<u8> {
    let trackId = track.id;

    // Track fragment header box
    let tfhd = genBox(
        b"tfhd",
        vec![
            0x00,
            0x00,
            0x00,
            0x00,                   // version(0) & flags
            (trackId >> 24) & 0xFF, // track_ID
            (trackId >> 16) & 0xFF,
            (trackId >> 8) & 0xFF,
            (trackId) & 0xFF,
        ],
    );
    // Track Fragment Decode Time
    let tfdt = genBox(
        b"tfdt",
        vec![
            0x00,
            0x00,
            0x00,
            0x00,                               // version(0) & flags
            (baseMediaDecodeTime >> 24) & 0xFF, // baseMediaDecodeTime: int32
            (baseMediaDecodeTime >> 16) & 0xFF,
            (baseMediaDecodeTime >> 8) & 0xFF,
            (baseMediaDecodeTime) & 0xFF,
        ],
    );
    let sdtp = sdtp(track);
    let trun = trun(track, sdtp.byteLength + 16 + 16 + 8 + 16 + 8 + 8);

    return genBox(b"traf", &[&tfhd, &tfdt, &trun, &sdtp]);
}

// Sample Dependency Type box
fn sdtp(track: &VideoTrack) -> Vec<u8> {
    let samples = track.samples || [];
    let sampleCount = samples.length;
    let data = Vec::with_capacity(4 + sampleCount);
    // 0~4 bytes: version(0) & flags
    for i in 0..sampleCount {
        let flags = samples[i].flags;
        data[i + 4] = (flags.isLeading << 6)    // is_leading: 2 (bit)
                    | (flags.dependsOn << 4)    // sample_depends_on
                    | (flags.isDependedOn << 2) // sample_is_depended_on
                    | (flags.hasRedundancy); // sample_has_redundancy
    }
    return genBox(b"sdtp", &[&data]);
}

// Track fragment run box
fn trun(track: &VideoTrack, offset: usize) -> Vec<u8> {
    let samples = track.samples || [];
    let sampleCount = samples.length;
    let dataSize = 12 + 16 * sampleCount;
    let data = Vec::with_capacity(dataSize);

    data.extend_from_slice(
        &[
            0x00,
            0x00,
            0x0F,
            0x01,                       // version(0) & flags
            (sampleCount >> 24) & 0xFF, // sample_count
            (sampleCount >> 16) & 0xFF,
            (sampleCount >> 8) & 0xFF,
            (sampleCount) & 0xFF,
            (offset >> 24) & 0xFF, // data_offset
            (offset >> 16) & 0xFF,
            (offset >> 8) & 0xFF,
            (offset) & 0xFF,
        ],
        0,
    );

    for i in 0..sampleCount {
        let duration = samples[i].duration;
        let size = samples[i].size;
        let flags = samples[i].flags;
        let cts = samples[i].cts;
        data.extend_from_slice(&[
            (duration >> 24) & 0xFF, // sample_duration
            (duration >> 16) & 0xFF,
            (duration >> 8) & 0xFF,
            (duration) & 0xFF,
            (size >> 24) & 0xFF, // sample_size
            (size >> 16) & 0xFF,
            (size >> 8) & 0xFF,
            (size) & 0xFF,
            (flags.isLeading << 2) | flags.dependsOn, // sample_flags
            (flags.isDependedOn << 6) | (flags.hasRedundancy << 4) | flags.isNonSync,
            0x00,
            0x00,               // sample_degradation_priority
            (cts >> 24) & 0xFF, // sample_composition_time_offset
            (cts >> 16) & 0xFF,
            (cts >> 8) & 0xFF,
            (cts) & 0xFF,
        ]);
    }
    return genBox(b"trun", &[&data]);
}

fn mdat(data: &[u8]) -> Vec<u8> {
    return genBox(b"mdat", data);
}
