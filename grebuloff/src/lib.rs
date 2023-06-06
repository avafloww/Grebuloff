use std::thread;
use msgbox::IconType;

dll_syringe::payload_procedure! {
    fn load(dir_vec: Vec<u8>, config_path: Vec<u8>) {
        load_inner(dir_vec, config_path)
    }
}

fn alert(message: &str) {
    msgbox::create("Grebuloff", message, IconType::Info).unwrap()
}

fn load_inner(dir_vec: Vec<u8>, config_path: Vec<u8>) {
    let dir = std::path::PathBuf::from(std::str::from_utf8(&dir_vec).unwrap());

    load_sync();

    // perform asynchronous loading in a different thread, after the framework thread resumes
    thread::spawn(load_async);
}

fn load_sync() {
    // do any early hooking (i.e. framework hooking here)
    // this is synchronous on the framework thread
}

fn load_async() {
    alert("Hello from Grebuloff!");
}