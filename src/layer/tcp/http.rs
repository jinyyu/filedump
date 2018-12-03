use layer::TCPDissector;
use std::rc::Rc;
use std::cell::RefCell;
use libc::c_char;
use detector::Detector;

pub struct HTTPDissector {
    detector: Rc<Detector>,
    flow: *const c_char,
}

impl HTTPDissector {
    pub fn new(detector: Rc<Detector>, flow: *const c_char) -> Rc<RefCell<TCPDissector>> {
        let http = HTTPDissector {
            detector,
            flow,
        };

        debug!("http request {}", http.detector.get_http_url(http.flow));

        Rc::new(RefCell::new(http))
    }
}

impl TCPDissector for HTTPDissector {
    fn on_client_data(&mut self, data: &[u8]) {
        trace!("http client data {}", data.len());
    }
    fn on_server_data(&mut self, data: &[u8]) {
        trace!("http server data {}", data.len());
    }
}