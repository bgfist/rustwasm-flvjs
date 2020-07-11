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
use crate::utils::logger::Log;
use byteorder::{BigEndian, ReadBytesExt};
use js_sys::Date;
use std::collections::hash_map::HashMap;
use std::io::Cursor;
use wasm_bindgen::prelude::*;

pub type ScriptData = HashMap<String, HashMap<String, AMFValue>>;

pub enum AMFValue {
    Undefined,
    Date(Date),
    Number(f64),
    Boolean(bool),
    Object(HashMap<String, AMFValue>),
    Array(Vec<AMFValue>),
}

struct Data<T> {
    data: T,
    size: usize,
    objectEnd: bool,
}

struct KeyValue {
    name: String,
    value: AMFValue,
}

pub fn parseScriptData(arrayBuffer: &[u8], dataOffset: usize, dataSize: usize) -> ScriptData {
    let data = HashMap::new();

    let name = parseValue(arrayBuffer, dataOffset, dataSize).data;
    let value = parseValue(arrayBuffer, dataOffset + name.size, dataSize - name.size).data;

    let (name, value) = match (name, value) {
        (AMFValue::String(s), AMFValue::Object(o)) => (s, o),
        _ => {
            return data;
        }
    };

    data.insert(name, value);

    return data;
}

fn parseObject(arrayBuffer: &[u8], dataOffset: usize, dataSize: usize) -> Data<KeyValue> {
    if dataSize < 3 {
        panic!("Data not enough when parse ScriptDataObject");
    }
    let name = parseString(arrayBuffer, dataOffset, dataSize);
    let value = parseValue(arrayBuffer, dataOffset + name.size, dataSize - name.size);
    let isObjectEnd = value.objectEnd;

    return Data {
        data: KeyValue {
            name: name.data,
            value: value.data,
        },
        size: name.size + value.size,
        objectEnd: isObjectEnd,
    };
}

fn parseVariable(arrayBuffer: &[u8], dataOffset: usize, dataSize: usize) -> Data<KeyValue> {
    return parseObject(arrayBuffer, dataOffset, dataSize);
}

fn parseString(arrayBuffer: &[u8], dataOffset: usize, dataSize: usize) -> Data<String> {
    if dataSize < 2 {
        panic!("Data not enough when parse String");
    }
    let v = Cursor::new(arrayBuffer[dataOffset..dataOffset + dataSize]);
    let length = v.read_u16().unwrap();

    let str;
    if length > 0 {
        str = String::from_utf8(&arrayBuffer[dataOffset + 2..dataOffset + 2 + length]).unwrap();
    } else {
        str = "".into();
    }

    return Data {
        data: str,
        size: 2 + length,
        objectEnd: false,
    };
}

fn parseLongString(arrayBuffer: &[u8], dataOffset: usize, dataSize: usize) -> Data<String> {
    if dataSize < 4 {
        panic!("Data not enough when parse LongString");
    }
    let v = Cursor::new(arrayBuffer[dataOffset..dataOffset + dataSize]);
    let length = v.read_u32::<BigEndian>().unwrap();

    let str;
    if length > 0 {
        str = String::from_utf8(&arrayBuffer[dataOffset + 4..dataOffset + 4 + length]).unwrap();
    } else {
        str = "".into();
    }

    return Data {
        data: str,
        size: 4 + length,
        objectEnd: false,
    };
}

fn parseDate(arrayBuffer: &[u8], dataOffset: usize, dataSize: usize) -> Data<Date> {
    if dataSize < 10 {
        panic!("Data size invalid when parse Date");
    }
    let v = Cursor::new(arrayBuffer[dataOffset..dataOffset + dataSize]);
    let timestamp = v.read_f64::<BigEndian>().unwrap();
    let localTimeOffset = v.read_u16::<BigEndian>().unwrap();
    timestamp += localTimeOffset * 60 * 1000; // get UTC time

    return Data {
        data: Date::new(JsValue::from(timestamp)),
        size: 8 + 2,
        objectEnd: false,
    };
}

fn parseValue(arrayBuffer: &[u8], dataOffset: usize, dataSize: usize) -> Data<AMFValue> {
    if dataSize < 1 {
        panic!("Data not enough when parse Value");
    }

    let v = Cursor::new(arrayBuffer[dataOffset..dataOffset + dataSize]);

    let offset = 1;
    let r#type = v.read_u8().unwrap();
    let value: AMFValue;
    let objectEnd = false;

    match r#type {
        0 => {
            // Number(Double) type
            value = AMFValue::Number(v.read_f64::<BigEndian>().unwrap());
            offset += 8;
        }
        1 => {
            // Boolean type
            let b = v.read_u8().unwrap();
            value = AMFValue::Boolean(b > 0);
            offset += 1;
        }
        2 => {
            // String type
            let amfstr = parseString(arrayBuffer, dataOffset + 1, dataSize - 1);
            value = AMFValue::String(amfstr.data);
            offset += amfstr.size;
        }
        3 => {
            // Object(s) type
            let map = HashMap::new();
            let terminal = 0; // workaround for malformed Objects which has missing ScriptDataObjectEnd

            v.set_position(dataSize - 3);
            if v.read_u24::<BigEndian>().unwrap() == 9 {
                terminal = 3;
            }
            while offset < dataSize - 4 {
                // 4 == type(UI8) + ScriptDataObjectEnd(UI24)
                let amfobj = parseObject(
                    arrayBuffer,
                    dataOffset + offset,
                    dataSize - offset - terminal,
                );
                if amfobj.objectEnd {
                    break;
                }
                map.insert(amfobj.data.name, amfobj.data.value);
                offset += amfobj.size;
            }
            if offset <= dataSize - 3 {
                v.set_position(offset);
                let marker = v.read_u24::<BigEndian>().unwrap();
                if marker == 9 {
                    offset += 3;
                }
            }
            value = AMFValue::Object(map);
        }
        8 => {
            // ECMA array type (Mixed array)
            let map = HashMap::new();
            offset += 4; // ECMAArrayLength(UI32)
            let terminal = 0; // workaround for malformed MixedArrays which has missing ScriptDataObjectEnd
            v.set_position(dataSize - 3);
            if v.read_u24::<BigEndian>().unwrap() == 9 {
                terminal = 3;
            }
            while offset < dataSize - 8 {
                // 8 == type(UI8) + ECMAArrayLength(UI32) + ScriptDataVariableEnd(UI24)
                let amfvar = parseVariable(
                    arrayBuffer,
                    dataOffset + offset,
                    dataSize - offset - terminal,
                );
                if amfvar.objectEnd {
                    break;
                }
                map.insert(amfvar.data.name, amfvar.data.value);
                offset += amfvar.size;
            }
            if offset <= dataSize - 3 {
                v.set_position(offset);
                let marker = v.read_u24::<BigEndian>().unwrap();
                if marker == 9 {
                    offset += 3;
                }
            }
            value = AMFValue::Object(map);
        }
        9 => {
            // ScriptDataObjectEnd
            value = AMFValue::Undefined;
            offset = 1;
            objectEnd = true;
        }
        10 => {
            // Strict array type
            // ScriptDataValue[n]. NOTE: according to video_file_format_spec_v10_1.pdf
            let strictArrayLength = v.read_u32::<BigEndian>().unwrap();
            let arr = Vec::with_capacity(strictArrayLength);
            offset += 4;
            for _ in 0..strictArrayLength {
                let val = parseValue(arrayBuffer, dataOffset + offset, dataSize - offset);
                arr.push(val.data);
                offset += val.size;
            }
            value = AMFValue::Array(arr);
        }
        11 => {
            // Date type
            let date = parseDate(arrayBuffer, dataOffset + 1, dataSize - 1);
            value = AMFValue::Date(date.data);
            offset += date.size;
        }
        12 => {
            // Long string type
            let amfLongStr = parseString(arrayBuffer, dataOffset + 1, dataSize - 1);
            value = AMFValue::String(amfLongStr.data);
            offset += amfLongStr.size;
        }
        _ => {
            // ignore and skip
            offset = dataSize;
            Log::w(
                "AMF",
                format!("{}{}", "Unsupported AMF value type ", r#type),
            );
        }
    }

    return Data {
        data: value,
        size: offset,
        objectEnd,
    };
}
