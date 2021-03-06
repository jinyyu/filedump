use aho_corasick::{AcAutomaton, Automaton};
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use yaml_rust::yaml;

unsafe impl Send for Configure {}

unsafe impl Sync for Configure {}

pub struct Configure {
    pub interface: String,
    pub workspace: String,
    pub worker_thread: i64,
    pub dissectors: HashMap<String, ()>,
    http_content_ac_automaton: Box<AcAutomaton<String>>,
}

lazy_static! {
    static ref CONFIG_PTR: AtomicPtr<Configure> = AtomicPtr::new(ptr::null_mut());
}

impl Configure {
    pub fn is_dissector_enable(&self, name: &str) -> bool {
        match self.dissectors.get(name) {
            Some(_) => true,
            None => false,
        }
    }

    pub fn is_parse_http_content(&self, content_type: &str) -> bool {
        if content_type.is_empty() {
            return false;
        }

        let mut it = self.http_content_ac_automaton.find(&*content_type);

        match it.next() {
            Some(_) => {
                return false;
            }
            None => {
                return true;
            }
        }
    }

    pub fn singleton() -> &'static Configure {
        let raw = CONFIG_PTR.load(Ordering::SeqCst);
        unsafe {
            return &*raw;
        }
    }
}

pub fn load(path: String) -> Box<Configure> {
    let mut f = File::open(path).unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();

    let docs = yaml::YamlLoader::load_from_str(&s).unwrap();
    assert_eq!(docs.capacity(), 1);
    let doc = &docs[0];

    let interface = doc["interface"].as_str().expect("invalid interface");
    info!("interface = {}", interface);

    let workspace = doc["workspace"].as_str().expect("invalid workspace");
    info!("workspace = {}", workspace);

    let worker_thread = doc["worker_thread"]
        .as_i64()
        .expect("invalid worker_thread");
    info!("worker_thread = {}", worker_thread);

    let mut skip_http_content_keys = Vec::new();
    for key in doc["skip_http_content_key"]
        .as_vec()
        .expect("invalid skip_http_content_key config")
        .iter()
    {
        let key = key.as_str().expect("invalid config");
        info!("skip http content key {}", key);
        skip_http_content_keys.push(key.to_string());
    }

    let http_content_ac_automaton = Box::new(AcAutomaton::new(skip_http_content_keys));

    let mut dissectors = HashMap::new();
    for dissector in doc["dissector"]
        .as_vec()
        .expect("invalid dissector config")
        .iter()
    {
        let dissector = dissector.as_str().expect("invalid config");
        info!("dissector {}", dissector);
        dissectors.insert(dissector.to_string(), ());
    }

    let conf = Box::new(Configure {
        interface: interface.to_string(),
        workspace: workspace.to_string(),
        worker_thread,
        dissectors,
        http_content_ac_automaton,
    });

    let raw = conf.as_ref() as *const Configure as *mut Configure;
    CONFIG_PTR.store(raw, Ordering::SeqCst);
    return conf;
}
