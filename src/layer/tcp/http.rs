use crate::detector::Detector;
use crate::layer::TCPDissector;
use libc::{c_char, c_void, free, malloc, strlen};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::CStr;
use std::io::prelude::*;
use std::mem;
use std::ptr;
use std::rc::Rc;
use std::slice;
use std::vec;

const REQUEST_SETTING: ParserSettings = ParserSettings {
    on_message_begin: on_request_message_begin,
    on_url,
    on_status,
    on_header_field: on_request_header_field,
    on_header_value: on_request_header_value,
    on_headers_complete: on_request_headers_complete,
    on_body: on_request_body,
    on_message_complete: on_request_message_complete,
    on_chunk_header,
    on_chunk_complete,
};

const RESPONSE_SETTING: ParserSettings = ParserSettings {
    on_message_begin: on_response_message_begin,
    on_url,
    on_status,
    on_header_field: on_response_header_field,
    on_header_value: on_response_header_value,
    on_headers_complete: on_response_headers_complete,
    on_body: on_response_body,
    on_message_complete: on_response_message_complete,
    on_chunk_header,
    on_chunk_complete,
};

#[repr(C)]
enum HttpParserType {
    Request,
    Response,
    _Both,
}

#[repr(C)]
struct Parser {
    opaque1: u32,
    nread: u32,
    content_length: u64,

    http_major: u16,
    http_minor: u16,
    status_code: u16,
    opaque2: u16,

    data: *const c_char,
}

type HTTPDataCallback =
extern "C" fn(_parser: *const Parser, _data: *const c_char, _length: isize) -> i32;

type HTTPCallback = extern "C" fn(_parser: *const Parser) -> i32;

#[repr(C)]
struct ParserSettings {
    on_message_begin: HTTPCallback,
    on_url: HTTPDataCallback,
    on_status: HTTPDataCallback,
    on_header_field: HTTPDataCallback,
    on_header_value: HTTPDataCallback,
    on_headers_complete: HTTPCallback,
    on_body: HTTPDataCallback,
    on_message_complete: HTTPCallback,
    on_chunk_header: HTTPCallback,
    on_chunk_complete: HTTPCallback,
}

extern "C" {
    fn http_parser_init(_parser: *mut Parser, _t: HttpParserType);
    fn http_parser_execute(
        _parser: *const Parser,
        _setting: *const ParserSettings,
        _data: *const c_char,
        _len: isize,
    ) -> isize;

    fn http_errno_description_from_parser(_parser: *const Parser) -> *const c_char;
}

extern "C" fn on_chunk_header(_parser: *const Parser) -> i32 {
    0
}

extern "C" fn on_chunk_complete(_parser: *const Parser) -> i32 {
    0
}

extern "C" fn on_request_message_begin(_parser: *const Parser) -> i32 {
    0
}

extern "C" fn on_url(parser: *const Parser, data: *const c_char, length: isize) -> i32 {
    let data = unsafe { slice::from_raw_parts(data as *const u8, length as usize) };

    let url = String::from_utf8_lossy(data).into_owned();

    unsafe {
        let this = (*parser).data as *mut HTTPDissector;
        if (*this).url.is_empty() {
            (*this).url = url;
        }
    }
    0
}

extern "C" fn on_request_header_field(
    parser: *const Parser,
    data: *const c_char,
    length: isize,
) -> i32 {
    unsafe {
        let data = slice::from_raw_parts(data as *const u8, length as usize);

        let header = String::from_utf8_lossy(data).into_owned();

        let this = (*parser).data as *mut HTTPDissector;
        (*this).request_header = header;
    }
    0
}

extern "C" fn on_request_header_value(
    parser: *const Parser,
    data: *const c_char,
    length: isize,
) -> i32 {
    unsafe {
        let data = slice::from_raw_parts(data as *const u8, length as usize);
        let value = String::from_utf8_lossy(data).into_owned();

        let this = (*parser).data as *mut HTTPDissector;
        let header = (*this).request_header.clone();

        (*this).request_headers.insert(header, value);
    }
    0
}

extern "C" fn on_request_headers_complete(parser: *const Parser) -> i32 {
    unsafe {
        let this = (*parser).data as *mut HTTPDissector;

        if (*this).content_type.is_empty() {
            let result = (*this).request_headers.get("Content-Type");
            match result {
                Some(value) => {
                    debug!("update Content-Type {}", value);
                    (*this).content_type = value.clone();
                }
                None => {
                    debug!("not Content-Type");
                }
            }
        }
    }
    0
}

extern "C" fn on_request_body(parser: *const Parser, data: *const c_char, length: isize) -> i32 {
    trace!("on_request_body");
    0
}

extern "C" fn on_request_message_complete(parser: *const Parser) -> i32 {
    trace!("on_request_message_complete");
    0
}

extern "C" fn on_response_message_begin(parser: *const Parser) -> i32 {
    trace!("on_response_message_begin");
    0
}

extern "C" fn on_status(parser: *const Parser, data: *const c_char, length: isize) -> i32 {
    unsafe {
        if (*parser).status_code != 200 {
            let s =
                String::from_utf8_lossy(slice::from_raw_parts(data as *const u8, length as usize))
                    .into_owned();
            trace!("http error : {} {}", (*parser).status_code, s);
        } else {}
    }
    0
}

extern "C" fn on_response_header_field(
    parser: *const Parser,
    data: *const c_char,
    length: isize,
) -> i32 {
    unsafe {
        let s = String::from_utf8_lossy(slice::from_raw_parts(data as *const u8, length as usize))
            .into_owned();
        let this = (*parser).data as *mut HTTPDissector;
        (*this).response_header = s;
    }
    0
}

extern "C" fn on_response_header_value(
    parser: *const Parser,
    data: *const c_char,
    length: isize,
) -> i32 {
    unsafe {
        let v = String::from_utf8_lossy(slice::from_raw_parts(data as *const u8, length as usize))
            .into_owned();

        let this = (*parser).data as *mut HTTPDissector;

        let k = (*this).response_header.clone();

        (*this).response_headers.insert(k, v);
    }
    0
}

extern "C" fn on_response_headers_complete(parser: *const Parser) -> i32 {
    unsafe {
        let this = (*parser).data as *mut HTTPDissector;

        if (*this).content_type.is_empty() {
            let result = (*this).response_headers.get("Content-Type");
            match result {
                Some(value) => {
                    debug!("update Content-Type {}", value);
                    (*this).content_type = value.clone();
                }
                None => {
                    debug!("not Content-Type");
                }
            }
        }
    }
    0
}

extern "C" fn on_response_body(parser: *const Parser, data: *const c_char, length: isize) -> i32 {
    trace!("on_response_body");
    0
}

extern "C" fn on_response_message_complete(parser: *const Parser) -> i32 {
    trace!("on_response_message_complete");
    0
}

pub struct HTTPDissector {
    url: String,
    content_type: String,
    request_header: String,
    request_headers: HashMap<String, String>,
    response_header: String,
    response_headers: HashMap<String, String>,
    buffer: Vec<u8>,
    request_parser: *const Parser,
    response_parser: *const Parser,
}

impl HTTPDissector {
    pub fn new(detector: Rc<Detector>, flow: *const c_char) -> Rc<RefCell<TCPDissector>> {
        let url = detector.get_http_url(flow);
        let content_type = detector.get_http_content_type(flow);
        trace!("url = {}, content_type = {}", url, content_type);
        let http = Rc::new(RefCell::new(HTTPDissector {
            url,
            content_type,
            request_header: "".to_string(),
            request_headers: HashMap::new(),
            response_header: "".to_string(),
            response_headers: HashMap::new(),
            buffer: Vec::new(),
            request_parser: ptr::null(),
            response_parser: ptr::null(),
        }));

        let this = http.as_ptr() as *const c_char;

        unsafe {
            let request_parser = malloc(mem::size_of::<Parser>()) as *mut Parser;
            http_parser_init(request_parser, HttpParserType::Request);
            (*request_parser).data = this;

            let response_parser = malloc(mem::size_of::<Parser>()) as *mut Parser;
            http_parser_init(response_parser, HttpParserType::Response);
            (*response_parser).data = this;

            http.borrow_mut().request_parser = request_parser;
            http.borrow_mut().response_parser = response_parser;
        }
        return http;
    }
}

impl Drop for HTTPDissector {
    fn drop(&mut self) {
        unsafe {
            free(self.request_parser as *mut c_void);
            free(self.response_parser as *mut c_void);
        }
    }
}

impl TCPDissector for HTTPDissector {
    fn on_client_data(&mut self, data: &[u8]) -> Result<(), ()> {
        unsafe {
            let n = http_parser_execute(
                self.request_parser,
                &REQUEST_SETTING as *const ParserSettings,
                data.as_ptr() as *const c_char,
                data.len() as isize,
            );

            if n != data.len() as isize {
                let c_str =
                    CStr::from_ptr(http_errno_description_from_parser(self.response_parser));
                let s = c_str.to_string_lossy();

                trace!("http parse error {}", s);
                Err(())
            } else {
                Ok(())
            }
        }
    }
    fn on_server_data(&mut self, data: &[u8]) -> Result<(), ()> {
        self.buffer.extend_from_slice(data);

        unsafe {
            let n = http_parser_execute(
                self.response_parser,
                &RESPONSE_SETTING as *const ParserSettings,
                data.as_ptr() as *const c_char,
                data.len() as isize,
            );

            if n != data.len() as isize {
                let err = http_errno_description_from_parser(self.response_parser);
                let c_str = CStr::from_bytes_with_nul_unchecked(slice::from_raw_parts(
                    err as *const u8,
                    strlen(err),
                ));
                let s = c_str.to_string_lossy();

                trace!("http parse error {}", s);
                Err(())
            } else {
                Ok(())
            }
        }
    }
}
