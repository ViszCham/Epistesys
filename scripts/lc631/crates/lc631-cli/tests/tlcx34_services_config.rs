//! Local configuration declarations cannot mint observations or permissions.
use lc631_cli::tlcx_commands::services::{read_registry, RegistryConfigError, CONFIG_RAM_BYTES};
use lc631_runtime::canonical_solve::{
    BackendId, LogicProfile, QueryResultKind, SolverProtocol, SolverRegistryError,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, Digest32, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use std::io::{Cursor, Read};
use std::time::Duration;

fn config() -> String {
    serde_json::json!({
        "schema_version":"epistesys-solver-registry.v1",
        "entries":[{
            "id":"z3", "profile":"smt_boolean_qf_lia", "protocol":"smt_lib_payload_wire_v2",
            "executable":if cfg!(windows) { "C:/registered/z3.exe" } else { "/registered/z3" },
            "version":"caller-declared-version", "binary_sha256":"11".repeat(32),
            "fixed_arguments":["-in","-smt2"],
            "caps":{
                "max_argv_count":8,"max_argv_bytes":4096,"max_stdin_bytes":16384,
                "max_stdout_bytes":65536,"max_stderr_bytes":16384,"max_timeout_ms":20000,
                "max_ram_bytes":8388608,"max_disk_bytes":0,"max_cleanup_bytes":1,
                "max_child_count":1
            }
        }]
    })
    .to_string()
}

fn check(input: &[u8]) -> Result<usize, RegistryConfigError> {
    let limits = ResourceLimits::uniform(CONFIG_RAM_BYTES).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            34,
            Digest32::from_bytes([34; 32]),
            limits,
            Deadline::after(Duration::from_secs(2)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(CONFIG_RAM_BYTES).unwrap(),
        )])
        .unwrap();
    let result = {
        let view = reservation.lease();
        read_registry(&mut Cursor::new(input), &view)
            .map(|loaded| loaded.registry().entries().len())
    };
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    result
}

#[test]
fn tlcx34_services_config_accepts_pins_without_opening_declared_executable() {
    assert_eq!(check(config().as_bytes()), Ok(1));
}

#[test]
fn tlcx34_services_config_rejects_unknown_fields_and_missing_caps() {
    let mut v: serde_json::Value = serde_json::from_str(&config()).unwrap();
    v["authority_created"] = true.into();
    assert!(check(v.to_string().as_bytes()).is_err());
    let mut nested: serde_json::Value = serde_json::from_str(&config()).unwrap();
    nested["entries"][0]["caps"]["authenticated"] = true.into();
    assert!(check(nested.to_string().as_bytes()).is_err());
    let duplicate = config().replacen(
        "\"schema_version\":",
        "\"schema_version\":\"epistesys-solver-registry.v1\",\"schema_version\":",
        1,
    );
    assert!(check(duplicate.as_bytes()).is_err());
    v.as_object_mut().unwrap().remove("authority_created");
    v["entries"][0]["caps"]
        .as_object_mut()
        .unwrap()
        .remove("max_child_count");
    assert!(check(v.to_string().as_bytes()).is_err());
}

#[test]
fn tlcx34_services_config_rejects_wrong_schema_protocol_digest_and_relative_path() {
    for (field, value) in [
        ("schema_version", "future.v2"),
        ("protocol", "smt_lib_payload_v1"),
        ("binary_sha256", "not-a-digest"),
        ("executable", "z3.exe"),
    ] {
        let mut v: serde_json::Value = serde_json::from_str(&config()).unwrap();
        if field == "schema_version" {
            v[field] = value.into();
        } else {
            v["entries"][0][field] = value.into();
        }
        assert!(check(v.to_string().as_bytes()).is_err(), "{field}");
    }
}

#[test]
fn tlcx34_services_config_rejects_enum_object_forms() {
    for (field, tag) in [
        ("schema_version", "epistesys-solver-registry.v1"),
        ("id", "z3"),
        ("profile", "smt_boolean_qf_lia"),
        ("protocol", "smt_lib_payload_wire_v2"),
        ("protocol", "smt_lib_status_v1"),
    ] {
        let mut value: serde_json::Value = serde_json::from_str(&config()).unwrap();
        let mut object = serde_json::Map::new();
        object.insert(tag.to_owned(), serde_json::Value::Null);
        let object = serde_json::Value::Object(object);
        if field == "schema_version" {
            value[field] = object;
        } else {
            value["entries"][0][field] = object;
        }
        assert!(check(value.to_string().as_bytes()).is_err(), "{field}");
    }
}

#[test]
fn tlcx34_services_config_accepts_explicit_status_and_rejects_its_duplicate() {
    let status = config().replace("smt_lib_payload_wire_v2", "smt_lib_status_v1");
    assert_eq!(check(status.as_bytes()), Ok(1));
    let mut value: serde_json::Value = serde_json::from_str(&status).unwrap();
    let duplicate = value["entries"][0].clone();
    value["entries"].as_array_mut().unwrap().push(duplicate);
    assert!(matches!(
        check(value.to_string().as_bytes()),
        Err(RegistryConfigError::Registry(
            SolverRegistryError::DuplicateRegistration
        ))
    ));
}

#[test]
fn tlcx34_services_config_explicit_protocol_pair_resolves_by_actual_query_kind() {
    let mut value: serde_json::Value = serde_json::from_str(&config()).unwrap();
    let mut status = value["entries"][0].clone();
    status["protocol"] = "smt_lib_status_v1".into();
    value["entries"].as_array_mut().unwrap().push(status);
    let input = value.to_string();
    let limits = ResourceLimits::uniform(CONFIG_RAM_BYTES).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            34,
            Digest32::from_bytes([34; 32]),
            limits,
            Deadline::after(Duration::from_secs(2)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(CONFIG_RAM_BYTES).unwrap(),
        )])
        .unwrap();
    {
        let view = reservation.lease();
        let loaded = read_registry(&mut Cursor::new(input.as_bytes()), &view).unwrap();
        let registry = loaded.registry();
        assert_eq!(registry.entries().len(), 2);
        assert_eq!(
            registry.resolve(BackendId::Z3, LogicProfile::SmtBooleanQfLia),
            Err(SolverRegistryError::AmbiguousRegistration)
        );
        for (kind, protocol) in [
            (
                QueryResultKind::Satisfiability,
                SolverProtocol::SmtLibStatusV1,
            ),
            (
                QueryResultKind::ModelPayload,
                SolverProtocol::SmtLibPayloadWireV2,
            ),
        ] {
            assert_eq!(
                registry
                    .resolve_for_query(BackendId::Z3, LogicProfile::SmtBooleanQfLia, kind,)
                    .unwrap()
                    .protocol(),
                protocol
            );
        }
        assert_eq!(
            registry.resolve_for_query(
                BackendId::Z3,
                LogicProfile::SmtBooleanQfLia,
                QueryResultKind::SpatialRelation,
            ),
            Err(SolverRegistryError::UnsupportedMapping)
        );
        assert!(!registry.completeness().is_complete());
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    value["entries"][1]["protocol"] = "future_protocol".into();
    assert!(check(value.to_string().as_bytes()).is_err());
}

#[test]
fn tlcx34_services_config_rejects_empty_duplicate_and_excess_entries() {
    let mut v: serde_json::Value = serde_json::from_str(&config()).unwrap();
    let entry = v["entries"][0].clone();
    for count in [0, 2, 17] {
        v["entries"] = vec![entry.clone(); count].into();
        assert!(check(v.to_string().as_bytes()).is_err(), "{count}");
    }
}

#[test]
fn tlcx34_services_config_rejects_invalid_caps_and_nul_arguments() {
    let mut v: serde_json::Value = serde_json::from_str(&config()).unwrap();
    v["entries"][0]["caps"]["max_child_count"] = 2.into();
    assert!(check(v.to_string().as_bytes()).is_err());
    v["entries"][0]["caps"]["max_child_count"] = 1.into();
    v["entries"][0]["fixed_arguments"] = vec!["\0"].into();
    assert!(check(v.to_string().as_bytes()).is_err());
}

#[test]
fn tlcx34_services_config_rejects_file_arguments_and_unsupported_disk_budget() {
    for arguments in [
        vec!["-in", "-smt2", "input.smt2"],
        vec!["-in", "-smt2", "-file:secret"],
        vec!["-smt2", "-in"],
        vec!["-in"],
    ] {
        let mut value: serde_json::Value = serde_json::from_str(&config()).unwrap();
        value["entries"][0]["fixed_arguments"] = arguments.into();
        assert!(check(value.to_string().as_bytes()).is_err());
    }
    let mut value: serde_json::Value = serde_json::from_str(&config()).unwrap();
    value["entries"][0]["caps"]["max_disk_bytes"] = 1.into();
    assert!(check(value.to_string().as_bytes()).is_err());
}

#[test]
fn tlcx34_services_config_bounds_actual_read_and_admits_before_reader_effect() {
    let mut padded = config().into_bytes();
    padded.resize(65536, b' ');
    assert_eq!(check(&padded), Ok(1));
    padded.push(b' ');
    assert!(check(&padded).is_err());
    struct NeverRead;
    impl Read for NeverRead {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            panic!("read before admission")
        }
    }
    let limits = ResourceLimits::uniform(1).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            34,
            Digest32::from_bytes([34; 32]),
            limits,
            Deadline::after(Duration::from_secs(2)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(1).unwrap(),
        )])
        .unwrap();
    {
        let view = reservation.lease();
        assert!(read_registry(&mut NeverRead, &view).is_err());
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx34_services_config_retains_owned_charge_and_limits_stream_consumption() {
    let limits = ResourceLimits::uniform(CONFIG_RAM_BYTES).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            34,
            Digest32::from_bytes([34; 32]),
            limits,
            Deadline::after(Duration::from_secs(2)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(CONFIG_RAM_BYTES).unwrap(),
        )])
        .unwrap();
    {
        let view = reservation.lease();
        let data = config();
        let loaded = read_registry(&mut Cursor::new(data.as_bytes()), &view).unwrap();
        let one = [ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(1).unwrap(),
        )];
        assert!(view.claim_resources(&one).is_err());
        assert_eq!(loaded.registry().entries().len(), 1);
        drop(loaded);
        drop(view.claim_resources(&one).unwrap());
        let mut stream = Cursor::new(vec![b' '; 1024 * 1024]);
        assert!(read_registry(&mut stream, &view).is_err());
        assert_eq!(stream.position(), 65537);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx34_services_config_checks_cancellation_between_reads() {
    struct Cancels {
        cancellation: Cancellation,
        reads: usize,
    }
    impl Read for Cancels {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            self.reads += 1;
            assert_eq!(self.reads, 1, "reader called again after cancellation");
            bytes[0] = b' ';
            self.cancellation.cancel();
            Ok(1)
        }
    }
    let limits = ResourceLimits::uniform(CONFIG_RAM_BYTES).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let cancellation = Cancellation::default();
    let mut run = broker
        .begin_run(
            34,
            Digest32::from_bytes([34; 32]),
            limits,
            Deadline::after(Duration::from_secs(2)).unwrap(),
            cancellation.clone(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(CONFIG_RAM_BYTES).unwrap(),
        )])
        .unwrap();
    {
        let view = reservation.lease();
        let mut reader = Cancels {
            cancellation,
            reads: 0,
        };
        assert!(matches!(
            read_registry(&mut reader, &view),
            Err(RegistryConfigError::Resource(
                lc631_runtime::resource::ResourceError::Cancelled
            ))
        ));
        assert_eq!(reader.reads, 1);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}
