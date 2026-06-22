use std::sync::atomic::Ordering;

use gts::object::{VirtualMachine, EXEC_MODE_BYTECODE};
use gts::runtime::Session;

#[test]
fn virtual_machine_new_defaults_to_bytecode_exec_mode() {
    let vm = VirtualMachine::new();
    assert_eq!(vm.exec_mode.load(Ordering::Relaxed), EXEC_MODE_BYTECODE);
}

#[test]
fn session_new_defaults_to_bytecode_exec_mode() {
    let session = Session::new();
    assert_eq!(
        session.vm().exec_mode.load(Ordering::Relaxed),
        EXEC_MODE_BYTECODE
    );
}
