/*
 * Copyright (C) 2016 Bilibili. All Rights Reserved.
 *
 * @author zheng qian <xqq@xqq.im>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use self file except in compliance with the License.
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
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn test(a: &[u8]) -> Vec<u8> {
    Vec::from(a)
}

#[wasm_bindgen]
extern "C" {
    fn alert(str: &str) -> String;
}

#[wasm_bindgen]
pub struct TestVec {
    arr: Vec<u8>
}

#[wasm_bindgen]
impl TestVec {
    #[wasm_bindgen(getter)]
    pub fn arr(self) -> Vec<u8> {
        self.arr
    }
}

#[wasm_bindgen]
#[derive(Serialize, Default)]
pub(crate) struct MediaInfo {
    mimeType: Option<String>,
    duration: Option<usize>,
    hasAudio: Option<bool>,
    hasVideo: Option<bool>,
    audioCodec: Option<String>,
    videoCodec: Option<String>,
    audioDataRate: Option<u32>,
    videoDataRate: Option<u32>,
    audioSampleRate: Option<u32>,
    audioChannelCount: Option<usize>,
    width: Option<usize>,
    height: Option<usize>,
    fps: Option<usize>,
    profile: Option<String>,
    level: Option<usize>,
    refFrames: Option<usize>,
    chromaFormat: Option<usize>,
    sarNum: Option<usize>,
    sarDen: Option<usize>,
    metadata: Option<u32>,
    segments: Option<u32>,
    segmentCount: Option<usize>,
    hasKeyframesIndex: Option<bool>,
    keyframesIndex: Option<usize>,
}

impl MediaInfo {
    pub fn isComplete(&self) -> bool {
        let audioInfoComplete = self.hasAudio.map_or(true, |hasAudio| {
            hasAudio
                && self.audioCodec.is_some()
                && self.audioSampleRate.is_some()
                && self.audioChannelCount.is_some()
        });

        let videoInfoComplete = self.hasVideo.map_or(true, |hasVideo| {
            hasVideo
                && self.videoCodec.is_some()
                && self.width.is_some()
                && self.height.is_some()
                && self.fps.is_some()
                && self.profile.is_some()
                && self.level.is_some()
                && self.refFrames.is_some()
                && self.chromaFormat.is_some()
                && self.sarNum.is_some()
                && self.sarDen.is_some()
        });

        // keyframesIndex may not be present
        return self.mimeType.is_some()
            && self.duration.is_some()
            && self.metadata.is_some()
            && self.hasKeyframesIndex.is_some()
            && audioInfoComplete
            && videoInfoComplete;
    }
}
