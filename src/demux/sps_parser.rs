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

use super::exp_golomb::ExpGolomb;
use js_sys::Math;
use wasm_bindgen::prelude::*;

fn _ebsp2rbsp(uint8array: &[u8]) -> Vec<u8> {
    let src = uint8array;
    let src_length = src.len();
    let dst = [0; src_length];
    let dst_idx = 0;

    for i in 0..src_length {
        if (i >= 2) {
            // Unescape: Skip 0x03 after 00 00
            if (src[i] == 0x03 && src[i - 1] == 0x00 && src[i - 2] == 0x00) {
                continue;
            }
        }
        dst[dst_idx] = src[i];
        dst_idx += 1;
    }

    return Vec::from(&dst[0..dst_idx]);
}

struct FrameInfo {
    fixed: u32,
    fps: u32,
    fps_den: u32,
    fps_num: u32,
}

struct Size {
    width: u32,
    height: u32,
}

struct SPSInfo {
    profile_string: &'static str, // baseline, high, high10, ...
    level_string: &'static str,   // 3, 3.1, 4, 4.1, 5, 5.1, ...
    bit_depth: u32,               // 8bit, 10bit, ...
    ref_frames: u32,
    chroma_format: u32, // 4:2:0, 4:2:2, ...
    chroma_format_string: &'static str,

    frame_rate: FrameInfo,

    sar_ratio: Size,

    codec_size: Size,

    present_size: Size,
}

pub fn parseSPS(uint8array: &[u8]) -> Result<SPSInfo, JsValue> {
    let rbsp = _ebsp2rbsp(uint8array);
    let gb = ExpGolomb::new(rbsp);

    gb.readByte()?;
    let profile_idc = gb.readByte(); // profile_idc
    gb.readByte()?; // constraint_set_flags[5] + reserved_zero[3]
    let level_idc = gb.readByte(); // level_idc
    gb.readUEG()?; // seq_parameter_set_id

    let profile_string = getProfileString(profile_idc);
    let level_string = getLevelString(level_idc);
    let chroma_format_idc = 1;
    let chroma_format = 420;
    let chroma_format_table = [0, 420, 422, 444];
    let bit_depth = 8;

    if (profile_idc == 100
        || profile_idc == 110
        || profile_idc == 122
        || profile_idc == 244
        || profile_idc == 44
        || profile_idc == 83
        || profile_idc == 86
        || profile_idc == 118
        || profile_idc == 128
        || profile_idc == 138
        || profile_idc == 144)
    {
        chroma_format_idc = gb.readUEG();
        if (chroma_format_idc == 3) {
            gb.readBits(1); // separate_colour_plane_flag
        }
        if (chroma_format_idc <= 3) {
            chroma_format = chroma_format_table[chroma_format_idc];
        }

        bit_depth = gb.readUEG() + 8; // bit_depth_luma_minus8
        gb.readUEG(); // bit_depth_chroma_minus8
        gb.readBits(1); // qpprime_y_zero_transform_bypass_flag
        if (gb.readBool()) {
            // seq_scaling_matrix_present_flag
            let scaling_list_count = if (chroma_format_idc != 3) { 8 } else { 12 };
            for i in 0..scaling_list_count {
                if (gb.readBool()) {
                    // seq_scaling_list_present_flag
                    if (i < 6) {
                        SPSParser._skipScalingList(gb, 16);
                    } else {
                        SPSParser._skipScalingList(gb, 64);
                    }
                }
            }
        }
    }
    gb.readUEG(); // log2_max_frame_num_minus4
    let pic_order_cnt_type = gb.readUEG();
    if (pic_order_cnt_type == 0) {
        gb.readUEG(); // log2_max_pic_order_cnt_lsb_minus_4
    } else if (pic_order_cnt_type == 1) {
        gb.readBits(1); // delta_pic_order_always_zero_flag
        gb.readSEG(); // offset_for_non_ref_pic
        gb.readSEG(); // offset_for_top_to_bottom_field
        let num_ref_frames_in_pic_order_cnt_cycle = gb.readUEG();
        for i in 0..num_ref_frames_in_pic_order_cnt_cycle {
            gb.readSEG(); // offset_for_ref_frame
        }
    }
    let ref_frames = gb.readUEG(); // max_num_ref_frames
    gb.readBits(1); // gaps_in_frame_num_value_allowed_flag

    let pic_width_in_mbs_minus1 = gb.readUEG();
    let pic_height_in_map_units_minus1 = gb.readUEG();

    let frame_mbs_only_flag = gb.readBits(1);
    if (frame_mbs_only_flag == 0) {
        gb.readBits(1); // mb_adaptive_frame_field_flag
    }
    gb.readBits(1); // direct_8x8_inference_flag

    let frame_crop_left_offset = 0;
    let frame_crop_right_offset = 0;
    let frame_crop_top_offset = 0;
    let frame_crop_bottom_offset = 0;

    let frame_cropping_flag = gb.readBool();
    if (frame_cropping_flag) {
        frame_crop_left_offset = gb.readUEG();
        frame_crop_right_offset = gb.readUEG();
        frame_crop_top_offset = gb.readUEG();
        frame_crop_bottom_offset = gb.readUEG();
    }

    let sar_width = 1;
    let sar_height = 1;
    let fps = 0;
    let fps_fixed = true;
    let fps_num = 0;
    let fps_den = 0;

    let vui_parameters_present_flag = gb.readBool();
    if (vui_parameters_present_flag) {
        if (gb.readBool()) {
            // aspect_ratio_info_present_flag
            let aspect_ratio_idc = gb.readByte();
            let sar_w_table = [1, 12, 10, 16, 40, 24, 20, 32, 80, 18, 15, 64, 160, 4, 3, 2];
            let sar_h_table = [1, 11, 11, 11, 33, 11, 11, 11, 33, 11, 11, 33, 99, 3, 2, 1];

            if (aspect_ratio_idc > 0 && aspect_ratio_idc < 16) {
                sar_width = sar_w_table[aspect_ratio_idc - 1];
                sar_height = sar_h_table[aspect_ratio_idc - 1];
            } else if (aspect_ratio_idc == 255) {
                sar_width = gb.readByte() << 8 | gb.readByte();
                sar_height = gb.readByte() << 8 | gb.readByte();
            }
        }

        if (gb.readBool()) {
            // overscan_info_present_flag
            gb.readBool(); // overscan_appropriate_flag
        }
        if (gb.readBool()) {
            // video_signal_type_present_flag
            gb.readBits(4); // video_format & video_full_range_flag
            if (gb.readBool()) {
                // colour_description_present_flag
                gb.readBits(24); // colour_primaries & transfer_characteristics & matrix_coefficients
            }
        }
        if (gb.readBool()) {
            // chroma_loc_info_present_flag
            gb.readUEG(); // chroma_sample_loc_type_top_field
            gb.readUEG(); // chroma_sample_loc_type_bottom_field
        }
        if (gb.readBool()) {
            // timing_info_present_flag
            let num_units_in_tick = gb.readBits(32);
            let time_scale = gb.readBits(32);
            fps_fixed = gb.readBool(); // fixed_frame_rate_flag

            fps_num = time_scale;
            fps_den = num_units_in_tick * 2;
            fps = fps_num / fps_den;
        }
    }

    let sarScale = 1;
    if (sar_width != 1 || sar_height != 1) {
        sarScale = sar_width / sar_height;
    }

    let crop_unit_x = 0;
    let crop_unit_y = 0;
    if (chroma_format_idc == 0) {
        crop_unit_x = 1;
        crop_unit_y = 2 - frame_mbs_only_flag;
    } else {
        let sub_wc = if (chroma_format_idc == 3) { 1 } else { 2 };
        let sub_hc = if (chroma_format_idc == 1) { 2 } else { 1 };
        crop_unit_x = sub_wc;
        crop_unit_y = sub_hc * (2 - frame_mbs_only_flag);
    }

    let codec_width = (pic_width_in_mbs_minus1 + 1) * 16;
    let codec_height = (2 - frame_mbs_only_flag) * ((pic_height_in_map_units_minus1 + 1) * 16);

    codec_width -= (frame_crop_left_offset + frame_crop_right_offset) * crop_unit_x;
    codec_height -= (frame_crop_top_offset + frame_crop_bottom_offset) * crop_unit_y;

    let present_width = Math::ceil(codec_width * sarScale);

    gb.destroy();
    gb = null;

    return SPSInfo {
        profile_string, // baseline, high, high10, ...
        level_string,   // 3, 3.1, 4, 4.1, 5, 5.1, ...
        bit_depth,      // 8bit, 10bit, ...
        ref_frames,
        chroma_format, // 4:2:0, 4:2:2, ...
        chroma_format_string: getChromaFormatString(chroma_format),

        frame_rate: FrameInfo {
            fixed: fps_fixed,
            fps,
            fps_den,
            fps_num,
        },

        sar_ratio: Size {
            width: sar_width,
            height: sar_height,
        },

        codec_size: Size {
            width: codec_width,
            height: codec_height,
        },

        present_size: Size {
            width: present_width,
            height: codec_height,
        },
    };
}

fn _skipScalingList(gb: ExpGolomb, count: u32) {
    let last_scale = 8;
    let next_scale = 8;
    let delta_scale = 0;

    for i in 0..count {
        if (next_scale != 0) {
            delta_scale = gb.readSEG();
            next_scale = (last_scale + delta_scale + 256) % 256;
        }
        last_scale = if (next_scale == 0) {
            last_scale
        } else {
            next_scale
        };
    }
}

pub fn getProfileString(profile_idc: u32) -> &'static str {
    match (profile_idc) {
        66 => return "Baseline",
        77 => return "Main",
        88 => return "Extended",
        100 => return "High",
        110 => return "High10",
        122 => return "High422",
        244 => return "High444",
        _ => return "Unknown",
    }
}

pub fn getLevelString(level_idc: u32) -> String {
    return (level_idc / 10).toFixed(1);
}

pub fn getChromaFormatString(chroma: u32) -> &'static str {
    match chroma {
        420 => return "4:2:0",
        422 => return "4:2:2",
        444 => return "4:4:4",
        _ => return "Unknown",
    }
}
