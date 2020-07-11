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

use crate::utils::exception::{IllegalStateException, InvalidArgumentException};
use byteorder::{BigEndian, ReadBytesExt};
use std::cmp::min;
use std::io::Cursor;
use wasm_bindgen::prelude::*;

pub struct ExpGolomb {
    TAG: &'static str,
    _buffer: Vec<u8>,
    _buffer_index: u32,
    _current_word: u32,
    _current_word_bits_left: u32,
}

// Exponential-Golomb buffer decoder
impl ExpGolomb {
    pub fn new(uint8array: Vec<u8>) -> ExpGolomb {
        ExpGolomb {
            TAG: "ExpGolomb",
            _buffer: uint8array,
            _buffer_index: 0,
            _current_word: 0,
            _current_word_bits_left: 0,
        }
    }

    fn _fillCurrentWord(&self) -> Result<(), JsValue> {
        if (self._buffer.len() <= 0) {
            return Err(IllegalStateException::new(
                "ExpGolomb: _fillCurrentWord() but no bytes available",
            ));
        }

        let bytes_read = min(4, self._buffer.len());
        let word: &[u8] = &self._buffer[self.__buffer_index..self._buffer_index + bytes_read];
        self._current_word = Cursor::new(word).read_u32::<BigEndian>();

        self._buffer_index += bytes_read;
        self._current_word_bits_left = bytes_read * 8;

        Ok()
    }

    fn readBits(&self, bits: u32) -> Result<u32, JsValue> {
        if (bits > 32) {
            return Err(InvalidArgumentException::new(
                "ExpGolomb: readBits() bits exceeded max 32bits!",
            ));
        }

        if (bits <= self._current_word_bits_left) {
            let result = self._current_word >> (32 - bits);
            self._current_word <<= bits;
            self._current_word_bits_left -= bits;
            return result;
        }

        let result = if self._current_word_bits_left != 0 {
            self._current_word
        } else {
            0
        };
        result = result >> (32 - self._current_word_bits_left);
        let bits_need_left = bits - self._current_word_bits_left;

        self._fillCurrentWord();
        let bits_read_next = min(bits_need_left, self._current_word_bits_left);

        let result2 = self._current_word >> (32 - bits_read_next);
        self._current_word <<= bits_read_next;
        self._current_word_bits_left -= bits_read_next;

        result = (result << bits_read_next) | result2;
        return Ok(result);
    }

    fn readBool(&self) -> Result<bool, JsValue> {
        return Ok(self.readBits(1)? == 1);
    }

    fn readByte(&self) -> Result<u32, JsValue> {
        return self.readBits(8);
    }

    fn _skipLeadingZero(&self) -> Result<u32, JsValue> {
        let zero_count: u32;
        for zero_count in 0..self._current_word_bits_left {
            if (0 != (self._current_word & (0x80000000 >> zero_count))) {
                self._current_word <<= zero_count;
                self._current_word_bits_left -= zero_count;
                return Ok(zero_count);
            }
        }

        self._fillCurrentWord()?;
        return Ok(zero_count + self._skipLeadingZero()?);
    }

    fn readUEG(&self) -> Result<u32, JsValue> {
        // unsigned exponential golomb
        let leading_zeros = self._skipLeadingZero()?;
        return self.readBits(leading_zeros + 1)? - 1;
    }

    fn readSEG(&self) -> Result<u32, JsValue> {
        // signed exponential golomb
        let value = self.readUEG()?;
        if (value & 0x01) {
            return Ok((value + 1) >> 1);
        } else {
            return Ok(-1 * (value >> 1));
        }
    }
}
