// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

use jni::objects::{GlobalRef, JByteArray, JClass, JObject, JObjectArray, JString, JValue};
use jni::sys::{jint, jlong, jobject};
use jni::{JNIEnv, JavaVM};
use paimon_ftindex_core::io::{ReadRequest, SeekRead, SeekWrite};
use paimon_ftindex_core::{FullTextIndexConfig, FullTextIndexReader, FullTextIndexWriter};
use std::collections::HashMap;
use std::io;
use std::ptr;

struct JavaOutput {
    jvm: JavaVM,
    output: GlobalRef,
}

unsafe impl Send for JavaOutput {}

impl SeekWrite for JavaOutput {
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        let mut env = self
            .jvm
            .attach_current_thread()
            .map_err(|e| io::Error::other(format!("JNI attach failed: {e}")))?;
        let array = env
            .new_byte_array(buf.len() as i32)
            .map_err(|e| io::Error::other(format!("new_byte_array failed: {e}")))?;
        let signed: Vec<i8> = buf.iter().map(|b| *b as i8).collect();
        env.set_byte_array_region(&array, 0, &signed)
            .map_err(|e| io::Error::other(format!("set_byte_array_region failed: {e}")))?;
        let array_obj = JObject::from(array);
        env.call_method(
            self.output.as_obj(),
            "write",
            "([BII)V",
            &[
                JValue::Object(&array_obj),
                JValue::Int(0),
                JValue::Int(buf.len() as i32),
            ],
        )
        .map_err(|e| io::Error::other(format!("Java output write failed: {e}")))?;
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut env = self
            .jvm
            .attach_current_thread()
            .map_err(|e| io::Error::other(format!("JNI attach failed: {e}")))?;
        env.call_method(self.output.as_obj(), "flush", "()V", &[])
            .map_err(|e| io::Error::other(format!("Java output flush failed: {e}")))?;
        Ok(())
    }
}

struct JavaInput {
    jvm: JavaVM,
    input: GlobalRef,
}

unsafe impl Send for JavaInput {}
unsafe impl Sync for JavaInput {}

impl SeekRead for JavaInput {
    fn pread(&self, ranges: &mut [ReadRequest<'_>]) -> io::Result<()> {
        let mut env = self
            .jvm
            .attach_current_thread()
            .map_err(|e| io::Error::other(format!("JNI attach failed: {e}")))?;

        for range in ranges {
            let position = i64::try_from(range.pos)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "offset overflow"))?;
            let length = i32::try_from(range.buf.len())
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "range too large"))?;
            let buffer = env
                .new_byte_array(length)
                .map_err(|e| io::Error::other(format!("new_byte_array failed: {e}")))?;
            let buffer_obj = JObject::from(buffer);
            env.call_method(
                self.input.as_obj(),
                "pread",
                "(J[BII)V",
                &[
                    JValue::Long(position),
                    JValue::Object(&buffer_obj),
                    JValue::Int(0),
                    JValue::Int(length),
                ],
            )
            .map_err(|e| io::Error::other(format!("Java input pread failed: {e}")))?;
            copy_java_buffer(&mut env, JByteArray::from(buffer_obj), range.buf)?;
        }
        Ok(())
    }
}

fn copy_java_buffer(
    env: &mut JNIEnv<'_>,
    buffer: JByteArray<'_>,
    output: &mut [u8],
) -> io::Result<()> {
    let length = env
        .get_array_length(&buffer)
        .map_err(|e| io::Error::other(format!("get_array_length failed: {e}")))?
        as usize;
    if length != output.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Java pread returned buffer length {} != {}",
                length,
                output.len()
            ),
        ));
    }
    if length > 0 {
        let mut signed = vec![0i8; output.len()];
        env.get_byte_array_region(&buffer, 0, &mut signed)
            .map_err(|e| io::Error::other(format!("get_byte_array_region failed: {e}")))?;
        for (dst, src) in output.iter_mut().zip(signed) {
            *dst = src as u8;
        }
    }
    Ok(())
}

struct WriterHandle {
    inner: FullTextIndexWriter,
}

struct ReaderHandle {
    inner: FullTextIndexReader<JavaInput>,
}

#[no_mangle]
pub extern "system" fn Java_org_apache_paimon_index_fulltext_FullTextNative_createWriter(
    mut env: JNIEnv,
    _class: JClass,
    keys: JObjectArray,
    values: JObjectArray,
) -> jlong {
    match create_writer(&mut env, keys, values) {
        Ok(ptr) => ptr,
        Err(e) => throw_and_return(&mut env, &e, 0),
    }
}

#[no_mangle]
pub extern "system" fn Java_org_apache_paimon_index_fulltext_FullTextNative_addDocument(
    mut env: JNIEnv,
    _class: JClass,
    writer_ptr: jlong,
    row_id: jlong,
    text: JString,
) {
    if let Err(e) = add_document(&mut env, writer_ptr, row_id, text) {
        throw(&mut env, &e);
    }
}

#[no_mangle]
pub extern "system" fn Java_org_apache_paimon_index_fulltext_FullTextNative_addDocumentFields(
    mut env: JNIEnv,
    _class: JClass,
    writer_ptr: jlong,
    row_id: jlong,
    field_names: JObjectArray,
    texts: JObjectArray,
) {
    if let Err(e) = add_document_fields(&mut env, writer_ptr, row_id, field_names, texts) {
        throw(&mut env, &e);
    }
}

#[no_mangle]
pub extern "system" fn Java_org_apache_paimon_index_fulltext_FullTextNative_writeIndex(
    mut env: JNIEnv,
    _class: JClass,
    writer_ptr: jlong,
    output: JObject,
) {
    if let Err(e) = write_index(&mut env, writer_ptr, output) {
        throw(&mut env, &e);
    }
}

#[no_mangle]
pub extern "system" fn Java_org_apache_paimon_index_fulltext_FullTextNative_freeWriter(
    _env: JNIEnv,
    _class: JClass,
    writer_ptr: jlong,
) {
    if writer_ptr != 0 {
        unsafe {
            drop(Box::from_raw(writer_ptr as *mut WriterHandle));
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_org_apache_paimon_index_fulltext_FullTextNative_openReader(
    mut env: JNIEnv,
    _class: JClass,
    input: JObject,
) -> jlong {
    match open_reader(&mut env, input) {
        Ok(ptr) => ptr,
        Err(e) => throw_and_return(&mut env, &e, 0),
    }
}

#[no_mangle]
pub extern "system" fn Java_org_apache_paimon_index_fulltext_FullTextNative_search(
    mut env: JNIEnv,
    _class: JClass,
    reader_ptr: jlong,
    query: JString,
    limit: jint,
) -> jobject {
    match search(&mut env, reader_ptr, query, limit, None) {
        Ok(obj) => obj,
        Err(e) => throw_and_return(&mut env, &e, ptr::null_mut()),
    }
}

#[no_mangle]
pub extern "system" fn Java_org_apache_paimon_index_fulltext_FullTextNative_searchWithRoaringFilter(
    mut env: JNIEnv,
    _class: JClass,
    reader_ptr: jlong,
    query: JString,
    limit: jint,
    roaring_filter: JByteArray,
) -> jobject {
    match search(&mut env, reader_ptr, query, limit, Some(roaring_filter)) {
        Ok(obj) => obj,
        Err(e) => throw_and_return(&mut env, &e, ptr::null_mut()),
    }
}

#[no_mangle]
pub extern "system" fn Java_org_apache_paimon_index_fulltext_FullTextNative_prewarm(
    mut env: JNIEnv,
    _class: JClass,
    reader_ptr: jlong,
) {
    if let Err(e) = prewarm(reader_ptr) {
        throw(&mut env, &e);
    }
}

#[no_mangle]
pub extern "system" fn Java_org_apache_paimon_index_fulltext_FullTextNative_readMetrics(
    mut env: JNIEnv,
    _class: JClass,
    reader_ptr: jlong,
) -> jobject {
    match read_metrics(&mut env, reader_ptr) {
        Ok(obj) => obj,
        Err(e) => throw_and_return(&mut env, &e, ptr::null_mut()),
    }
}

#[no_mangle]
pub extern "system" fn Java_org_apache_paimon_index_fulltext_FullTextNative_freeReader(
    _env: JNIEnv,
    _class: JClass,
    reader_ptr: jlong,
) {
    if reader_ptr != 0 {
        unsafe {
            drop(Box::from_raw(reader_ptr as *mut ReaderHandle));
        }
    }
}

fn create_writer(
    env: &mut JNIEnv,
    keys: JObjectArray,
    values: JObjectArray,
) -> Result<jlong, String> {
    let options = options_from_arrays(env, keys, values)?;
    let config = FullTextIndexConfig::from_options(&options).map_err(|e| e.to_string())?;
    let writer = FullTextIndexWriter::new(config).map_err(|e| e.to_string())?;
    Ok(Box::into_raw(Box::new(WriterHandle { inner: writer })) as jlong)
}

fn add_document(
    env: &mut JNIEnv,
    writer_ptr: jlong,
    row_id: jlong,
    text: JString,
) -> Result<(), String> {
    let writer = handle_mut::<WriterHandle>(writer_ptr, "writer")?;
    let text: String = env
        .get_string(&text)
        .map_err(|e| format!("failed to read text: {e}"))?
        .into();
    writer
        .inner
        .add_document(row_id, text)
        .map_err(|e| e.to_string())
}

fn add_document_fields(
    env: &mut JNIEnv,
    writer_ptr: jlong,
    row_id: jlong,
    field_names: JObjectArray,
    texts: JObjectArray,
) -> Result<(), String> {
    let writer = handle_mut::<WriterHandle>(writer_ptr, "writer")?;
    let fields = string_pair_arrays(env, field_names, texts)?;
    writer
        .inner
        .add_document_fields(row_id, fields)
        .map_err(|e| e.to_string())
}

fn write_index(env: &mut JNIEnv, writer_ptr: jlong, output: JObject) -> Result<(), String> {
    let writer = handle_mut::<WriterHandle>(writer_ptr, "writer")?;
    let jvm = env.get_java_vm().map_err(|e| e.to_string())?;
    let output = env.new_global_ref(output).map_err(|e| e.to_string())?;
    let mut output = JavaOutput { jvm, output };
    writer.inner.write(&mut output).map_err(|e| e.to_string())
}

fn open_reader(env: &mut JNIEnv, input: JObject) -> Result<jlong, String> {
    let jvm = env.get_java_vm().map_err(|e| e.to_string())?;
    let input = env.new_global_ref(input).map_err(|e| e.to_string())?;
    let input = JavaInput { jvm, input };
    let reader = FullTextIndexReader::open(input).map_err(|e| e.to_string())?;
    Ok(Box::into_raw(Box::new(ReaderHandle { inner: reader })) as jlong)
}

fn search(
    env: &mut JNIEnv,
    reader_ptr: jlong,
    query: JString,
    limit: jint,
    roaring_filter: Option<JByteArray>,
) -> Result<jobject, String> {
    let reader = handle_ref::<ReaderHandle>(reader_ptr, "reader")?;
    let query: String = env
        .get_string(&query)
        .map_err(|e| format!("failed to read query: {e}"))?
        .into();
    let limit = validate_search_limit(limit)?;
    let result = if let Some(roaring_filter) = roaring_filter {
        let roaring_filter = read_byte_array(env, roaring_filter)?;
        reader
            .inner
            .search_with_roaring_filter(&query, limit, &roaring_filter)
            .map_err(|e| e.to_string())?
    } else {
        reader
            .inner
            .search(&query, limit)
            .map_err(|e| e.to_string())?
    };

    let row_ids = env
        .new_long_array(result.row_ids.len() as i32)
        .map_err(|e| e.to_string())?;
    env.set_long_array_region(&row_ids, 0, &result.row_ids)
        .map_err(|e| e.to_string())?;
    let scores = env
        .new_float_array(result.scores.len() as i32)
        .map_err(|e| e.to_string())?;
    env.set_float_array_region(&scores, 0, &result.scores)
        .map_err(|e| e.to_string())?;

    let row_ids_obj = JObject::from(row_ids);
    let scores_obj = JObject::from(scores);
    let obj = env
        .new_object(
            "org/apache/paimon/index/fulltext/FullTextSearchResult",
            "([J[F)V",
            &[JValue::Object(&row_ids_obj), JValue::Object(&scores_obj)],
        )
        .map_err(|e| e.to_string())?;
    Ok(obj.into_raw())
}

fn prewarm(reader_ptr: jlong) -> Result<(), String> {
    let reader = handle_ref::<ReaderHandle>(reader_ptr, "reader")?;
    reader.inner.prewarm().map_err(|e| e.to_string())
}

fn read_metrics(env: &mut JNIEnv, reader_ptr: jlong) -> Result<jobject, String> {
    let reader = handle_ref::<ReaderHandle>(reader_ptr, "reader")?;
    let metrics = reader.inner.read_metrics();
    let obj = env
        .new_object(
            "org/apache/paimon/index/fulltext/FullTextReadMetrics",
            "(JJJJJJJ)V",
            &[
                JValue::Long(metrics.pread_calls as jlong),
                JValue::Long(metrics.pread_ranges as jlong),
                JValue::Long(metrics.pread_bytes as jlong),
                JValue::Long(metrics.cache_hits as jlong),
                JValue::Long(metrics.cache_misses as jlong),
                JValue::Long(metrics.cache_evictions as jlong),
                JValue::Long(metrics.cached_blocks as jlong),
            ],
        )
        .map_err(|e| e.to_string())?;
    Ok(obj.into_raw())
}

fn validate_search_limit(limit: jint) -> Result<usize, String> {
    if limit <= 0 {
        return Err("search limit must be positive".to_string());
    }
    Ok(limit as usize)
}

fn read_byte_array(env: &mut JNIEnv, array: JByteArray) -> Result<Vec<u8>, String> {
    if array.as_raw().is_null() {
        return Err("roaringFilter is null".to_string());
    }
    env.convert_byte_array(array)
        .map_err(|e| format!("failed to read roaringFilter: {e}"))
}

fn options_from_arrays(
    env: &mut JNIEnv,
    keys: JObjectArray,
    values: JObjectArray,
) -> Result<HashMap<String, String>, String> {
    let key_len = env.get_array_length(&keys).map_err(|e| e.to_string())?;
    let value_len = env.get_array_length(&values).map_err(|e| e.to_string())?;
    if key_len != value_len {
        return Err(format!(
            "keys length {} does not match values length {}",
            key_len, value_len
        ));
    }
    let mut options = HashMap::with_capacity(key_len as usize);
    for i in 0..key_len {
        let key = env
            .get_object_array_element(&keys, i)
            .map_err(|e| e.to_string())?;
        let value = env
            .get_object_array_element(&values, i)
            .map_err(|e| e.to_string())?;
        let key: String = env
            .get_string(&JString::from(key))
            .map_err(|e| e.to_string())?
            .into();
        let value: String = env
            .get_string(&JString::from(value))
            .map_err(|e| e.to_string())?
            .into();
        options.insert(key, value);
    }
    Ok(options)
}

fn string_pair_arrays(
    env: &mut JNIEnv,
    keys: JObjectArray,
    values: JObjectArray,
) -> Result<Vec<(String, String)>, String> {
    let key_len = env.get_array_length(&keys).map_err(|e| e.to_string())?;
    let value_len = env.get_array_length(&values).map_err(|e| e.to_string())?;
    if key_len != value_len {
        return Err(format!(
            "fieldNames length {} does not match texts length {}",
            key_len, value_len
        ));
    }
    if key_len == 0 {
        return Err("document fields must not be empty".to_string());
    }
    let mut fields = Vec::with_capacity(key_len as usize);
    for i in 0..key_len {
        let key = env
            .get_object_array_element(&keys, i)
            .map_err(|e| e.to_string())?;
        let value = env
            .get_object_array_element(&values, i)
            .map_err(|e| e.to_string())?;
        let key: String = env
            .get_string(&JString::from(key))
            .map_err(|e| e.to_string())?
            .into();
        let value: String = env
            .get_string(&JString::from(value))
            .map_err(|e| e.to_string())?
            .into();
        fields.push((key, value));
    }
    Ok(fields)
}

fn handle_mut<'a, T>(ptr: jlong, name: &str) -> Result<&'a mut T, String> {
    if ptr == 0 {
        return Err(format!("{name} is closed"));
    }
    unsafe {
        (ptr as *mut T)
            .as_mut()
            .ok_or_else(|| format!("{name} is null"))
    }
}

fn handle_ref<'a, T>(ptr: jlong, name: &str) -> Result<&'a T, String> {
    if ptr == 0 {
        return Err(format!("{name} is closed"));
    }
    unsafe {
        (ptr as *const T)
            .as_ref()
            .ok_or_else(|| format!("{name} is null"))
    }
}

fn throw(env: &mut JNIEnv, message: &str) {
    let _ = env.throw_new("java/lang/RuntimeException", message);
}

fn throw_and_return<T>(env: &mut JNIEnv, message: &str, value: T) -> T {
    throw(env, message);
    value
}

#[cfg(test)]
mod tests {
    use super::validate_search_limit;

    #[test]
    fn validates_search_limit_before_usize_cast() {
        assert_eq!(validate_search_limit(1), Ok(1));
        assert!(validate_search_limit(0).is_err());
        assert!(validate_search_limit(-1).is_err());
    }
}
