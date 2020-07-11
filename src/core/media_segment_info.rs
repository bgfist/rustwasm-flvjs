use js_sys::Array;
use wasm_bindgen::prelude::*;

// Represents an media sample (audio / video)
#[wasm_bindgen]
#[derive(Copy, Clone)]
pub struct SampleInfo {
    pub dts: u32,
    pub pts: u32,
    pub duration: u32,
    pub originalDts: u32,
    pub isSyncPoint: bool,
    pub fileposition: usize,
}

impl SampleInfo {
    fn new(dts: u32, pts: u32, duration: u32, originalDts: u32, isSyncPoint: bool) -> SampleInfo {
        return SampleInfo {
            dts,
            pts,
            duration,
            originalDts,
            isSyncPoint,
            fileposition: 0,
        };
    }
}

// Media Segment concept is defined in Media Source Extensions spec.
// Particularly in ISO BMFF format, an Media Segment contains a moof box followed by a mdat box.
#[wasm_bindgen]
pub struct MediaSegmentInfo {
    pub beginDts: usize,
    pub endDts: usize,
    pub beginPts: usize,
    pub endPts: usize,
    pub originalEndDts: usize,
    syncPoints: Vec<SampleInfo>,
    pub firstSample: Option<SampleInfo>,
    pub lastSample: Option<SampleInfo>,
}

#[wasm_bindgen]
impl MediaSegmentInfo {
    #[wasm_bindgen(getter)]
    pub fn syncPoints(self) -> Array {
        self.syncPoints.into_iter().map(JsValue::from).collect()
    }
}

impl MediaSegmentInfo {
    fn appendSyncPoint(&mut self, mut sampleInfo: SampleInfo) {
        // also called Random Access Point
        sampleInfo.isSyncPoint = true;
        self.syncPoints.push(sampleInfo);
    }
}

// Data structure for recording information of media segments in single track.
pub struct MediaSegmentInfoList {
    r#type: &'static str,
    _list: Vec<MediaSegmentInfo>,
    _lastAppendLocation: isize,
}

impl MediaSegmentInfoList {
    fn new(r#type: &'static str) -> MediaSegmentInfoList {
        MediaSegmentInfoList {
            r#type,
            _list: vec![],
            _lastAppendLocation: -1, // cached last insert location
        }
    }

    fn r#type(&self) -> &'static str {
        self.r#type
    }

    fn length(&self) -> usize {
        self._list.len()
    }

    fn isEmpty(&self) -> bool {
        self._list.is_empty()
    }

    fn clear(&self) {
        self._list.clear();
        self._lastAppendLocation = -1;
    }

    fn _searchNearestSegmentBefore(&self, originalBeginDts: u32) -> isize {
        let list = self._list;
        if (list.is_empty()) {
            return -2;
        }
        let last = list.len() - 1;
        let mid = 0;
        let lbound = 0;
        let ubound = last;

        let idx = 0;

        if (originalBeginDts < list[0].originalBeginDts) {
            idx = -1;
            return idx;
        }

        while (lbound <= ubound) {
            mid = lbound + Math.floor((ubound - lbound) / 2);
            if (mid == last
                || (originalBeginDts > list[mid].lastSample.originalDts
                    && (originalBeginDts < list[mid + 1].originalBeginDts)))
            {
                idx = mid;
                break;
            } else if (list[mid].originalBeginDts < originalBeginDts) {
                lbound = mid + 1;
            } else {
                ubound = mid - 1;
            }
        }
        return idx;
    }

    fn _searchNearestSegmentAfter(&self, originalBeginDts: u32) {
        return self._searchNearestSegmentBefore(originalBeginDts) + 1;
    }

    fn append(&self, mediaSegmentInfo: MediaSegmentInfo) {
        let list = self._list;
        let msi = mediaSegmentInfo;
        let lastAppendIdx = self._lastAppendLocation;
        let insertIdx = 0;

        if (lastAppendIdx != -1
            && lastAppendIdx < list.length
            && msi.originalBeginDts >= list[lastAppendIdx].lastSample.originalDts
            && ((lastAppendIdx == list.length - 1)
                || (lastAppendIdx < list.length - 1
                    && msi.originalBeginDts < list[lastAppendIdx + 1].originalBeginDts)))
        {
            insertIdx = lastAppendIdx + 1; // use cached location idx
        } else {
            if (list.length > 0) {
                insertIdx = self._searchNearestSegmentBefore(msi.originalBeginDts) + 1;
            }
        }

        self._lastAppendLocation = insertIdx;
        self._list.splice(insertIdx, 0, msi);
    }

    fn getLastSegmentBefore(&self, originalBeginDts: u32) -> Option<&MediaSegmentInfo> {
        let idx = self._searchNearestSegmentBefore(originalBeginDts);
        if idx >= 0 {
            return Some(self._list.get(idx as usize)?);
        } else {
            // -1
            return None;
        }
    }

    fn getLastSampleBefore(&self, originalBeginDts: u32) -> Option<&SampleInfo> {
        let segment = self.getLastSegmentBefore(originalBeginDts);

        segment.and_then(|segment| segment.lastSample.as_ref())
    }

    fn getLastSyncPointBefore(&self, originalBeginDts: u32) -> Option<u32> {
        let segmentIdx = self._searchNearestSegmentBefore(originalBeginDts);
        let syncPoints = self._list.get(segmentIdx)?.syncPoints;
        while (syncPoints.length == 0 && segmentIdx > 0) {
            segmentIdx -= 1;
            syncPoints = self._list[segmentIdx].syncPoints;
        }
        if (syncPoints.length > 0) {
            return Some(syncPoints[syncPoints.length - 1]);
        } else {
            return None;
        }
    }
}
