use gts::object::{AsyncCompletionData, AsyncCompletionResult, PromiseState, VirtualMachine};

#[test]
fn background_thread_can_enqueue_owned_completion_for_vm_drain() {
    let vm = VirtualMachine::new();
    let sender = vm.async_completion_sender();
    let id = vm.next_async_completion_id();

    vm.async_add(1);
    let worker = std::thread::spawn(move || {
        sender.resolve(id, AsyncCompletionData::Text("done".to_string()));
    });
    worker.join().unwrap();

    let completions = vm.drain_async_completions();
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].id, id);
    assert_eq!(
        completions[0].result,
        AsyncCompletionResult::Resolve(AsyncCompletionData::Text("done".to_string()))
    );
    assert_eq!(vm.drain_async_completions(), Vec::new());
}

#[test]
fn vm_enqueue_helpers_support_reject_and_resolve() {
    let vm = VirtualMachine::new();
    let resolve_id = vm.next_async_completion_id();
    let reject_id = vm.next_async_completion_id();

    vm.async_add(2);
    vm.enqueue_async_resolve(
        resolve_id,
        AsyncCompletionData::JsonText("{\"ok\":true}".into()),
    );
    vm.enqueue_async_reject(reject_id, "network failed");

    let completions = vm.drain_async_completions();
    assert_eq!(completions.len(), 2);
    assert!(matches!(
        completions[0].result,
        AsyncCompletionResult::Resolve(AsyncCompletionData::JsonText(_))
    ));
    assert_eq!(
        completions[1].result,
        AsyncCompletionResult::Reject("network failed".into())
    );
}

#[test]
fn vm_drain_settles_registered_promise_on_vm_thread() {
    let vm = VirtualMachine::new();
    let (id, promise) = vm.create_async_completion_promise();

    assert_eq!(promise.state(), PromiseState::Pending);
    assert_eq!(vm.async_registered_promise_len(), 1);

    vm.enqueue_async_resolve(id, AsyncCompletionData::Text("settled".to_string()));

    let completions = vm.drain_async_completions();
    assert_eq!(completions.len(), 1);
    assert_eq!(vm.async_registered_promise_len(), 0);
    assert_eq!(promise.state(), PromiseState::Fulfilled);
    assert_eq!(promise.wait().inspect(), "settled");
}

#[test]
fn vm_drain_rejects_registered_promise_on_vm_thread() {
    let vm = VirtualMachine::new();
    let (id, promise) = vm.create_async_completion_promise();

    vm.enqueue_async_reject(id, "network failed");

    let completions = vm.drain_async_completions();
    assert_eq!(completions.len(), 1);
    assert_eq!(promise.state(), PromiseState::Rejected);
    assert!(promise.wait().inspect().contains("network failed"));
}

#[test]
fn wait_async_wakes_when_registered_completion_arrives() {
    let vm = VirtualMachine::new();
    let (id, promise) = vm.create_async_completion_promise();
    let sender = vm.async_completion_sender();

    let worker = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(10));
        sender.resolve(id, AsyncCompletionData::Text("awake".to_string()));
    });

    vm.wait_async();
    worker.join().unwrap();

    assert!(!vm.has_async_pending());
    assert_eq!(promise.state(), PromiseState::Fulfilled);
    assert_eq!(promise.wait().inspect(), "awake");
}

#[cfg(feature = "tokio")]
#[test]
fn tokio_task_can_enqueue_completion_for_vm_drain() {
    use gts::async_runtime::TokioRuntime;

    let vm = VirtualMachine::new();
    let runtime = TokioRuntime::new();
    let sender = vm.async_completion_sender();
    let id = vm.next_async_completion_id();

    vm.async_add(1);
    let handle = runtime.spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        sender.resolve(id, AsyncCompletionData::Bytes(vec![1, 2, 3]));
    });

    runtime.block_on(handle).unwrap();

    let completions = vm.drain_async_completions();
    assert_eq!(completions.len(), 1);
    assert_eq!(
        completions[0].result,
        AsyncCompletionResult::Resolve(AsyncCompletionData::Bytes(vec![1, 2, 3]))
    );
}
