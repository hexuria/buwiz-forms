use super::*;

fn module() -> HostModule {
    HostModule::new("demo")
        .function("echo", |arguments| Ok(arguments.value(0)?.clone()))
        .async_function("later", |arguments| {
            let n = arguments.integer(0)?;
            Ok(async move { Ok(HostValue::from(n * 2)) })
        })
}

#[test]
fn functions_list_names_with_their_kind_sorted() {
    let module = module().function("alpha", |_| Ok(HostValue::Null));
    assert_eq!(
        module.functions(),
        vec![
            ("alpha", FunctionKind::Sync),
            ("echo", FunctionKind::Sync),
            ("later", FunctionKind::Async),
        ]
    );
    assert_eq!(module.function_names(), vec!["alpha", "echo", "later"]);
}

#[test]
fn one_name_has_one_kind() {
    let module = module().function("later", |_| Ok(HostValue::Null));
    assert!(!module.is_async("later"));
    assert!(module.has("later"));
    assert_eq!(module.functions().len(), 2);
}

#[test]
fn call_and_begin_dispatch_by_kind() {
    let module = module();
    assert_eq!(
        module.call("echo", &HostArguments::new([HostValue::from("hi")])),
        Ok(HostValue::from("hi"))
    );
    let future = module
        .begin("later", &HostArguments::new([HostValue::from(21)]))
        .expect("argument accepted");
    assert_eq!(futures_lite_block_on(future), Ok(HostValue::from(42)));
    let error = module.call("later", &HostArguments::default()).unwrap_err();
    assert_eq!(
        error.message(),
        "HostModule `demo` has no function `later`; it provides: echo, later"
    );
    let error = module
        .begin("later", &HostArguments::default())
        .err()
        .unwrap();
    assert_eq!(error.message(), "argument 1 is missing; 0 were passed");
}

#[test]
fn argument_readers_name_the_position() {
    let arguments = HostArguments::new([HostValue::from(1.5), HostValue::Null]);
    assert_eq!(
        arguments.integer(0).unwrap_err().message(),
        "argument 1 must be a whole number, got 1.5"
    );
    assert_eq!(
        arguments.string(1).unwrap_err().message(),
        "argument 2 must be a string, got null"
    );
    assert_eq!(arguments.len(), 2);
    assert!(!arguments.is_empty());
}

#[test]
fn validate_compares_declarations_and_refuses_reserved_names() {
    let declared = module().declarations(
        "export function echo(v: unknown): unknown;\nexport function later(n: number): Promise<number>;",
    );
    assert!(declared.validate().is_ok());
    assert_eq!(
        declared.declared(),
        Some(
            "export function echo(v: unknown): unknown;\nexport function later(n: number): Promise<number>;"
        )
    );
    let error = module()
        .declarations("export function echo(v: unknown): unknown;\nexport const extra: number;")
        .validate()
        .unwrap_err();
    assert_eq!(
        error.message(),
        "HostModule `demo` declares a different set of exports than it registers; \
         registered but not declared: later; declared but not registered: extra"
    );
    assert!(HostModule::new("path").validate().is_err());
}

#[test]
fn host_object_keeps_order_and_replaces_repeats() {
    let value: HostValue = HostObject::new()
        .field("b", 1)
        .field("a", true)
        .field("b", "two")
        .into();
    assert_eq!(
        value,
        HostValue::Object(vec![
            ("b".into(), HostValue::from("two")),
            ("a".into(), HostValue::Bool(true)),
        ])
    );
    assert_eq!(value.get("a"), Some(&HostValue::Bool(true)));
}

/// Polls a future that is ready without waking (the test futures above).
fn futures_lite_block_on(mut future: HostFuture) -> HostResult {
    use std::task::{Context, Poll, Waker};
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("test future was not ready"),
    }
}
