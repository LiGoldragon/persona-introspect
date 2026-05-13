use std::os::unix::fs::PermissionsExt;
use std::thread;

use persona_introspect::daemon::{IntrospectionDaemon, IntrospectionSignalClient, SocketMode};
use signal_persona_auth::EngineId;
use signal_persona_introspect::{
    ComponentReadiness, ComponentSnapshotQuery, CorrelationId, DeliveryTraceQuery,
    DeliveryTraceStatus, EngineSnapshotQuery, IntrospectionReply, IntrospectionRequest,
    IntrospectionTarget, PrototypeWitnessQuery,
};

fn serve_one(request: IntrospectionRequest) -> IntrospectionReply {
    let directory = tempfile::tempdir().expect("tempdir");
    let socket = directory.path().join("introspect.sock");
    let bound = IntrospectionDaemon::from_socket(socket.clone())
        .with_socket_mode(SocketMode::from_octal(0o600))
        .bind()
        .expect("daemon binds");
    assert_eq!(
        std::fs::metadata(bound.socket())
            .expect("socket metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );

    let server = thread::spawn(move || bound.serve_one().expect("serve one"));
    let reply = IntrospectionSignalClient::new(socket)
        .submit(request)
        .expect("client receives reply");
    let served = server.join().expect("server joins");
    assert_eq!(served, reply);
    reply
}

#[test]
fn daemon_serves_prototype_witness_over_signal_socket() {
    let reply = serve_one(IntrospectionRequest::PrototypeWitness(
        PrototypeWitnessQuery {
            engine: EngineId::new("prototype"),
        },
    ));

    match reply {
        IntrospectionReply::PrototypeWitness(witness) => {
            assert_eq!(witness.engine, EngineId::new("prototype"));
            assert_eq!(witness.manager_seen, ComponentReadiness::Unknown);
            assert_eq!(witness.router_seen, ComponentReadiness::Unknown);
            assert_eq!(witness.terminal_seen, ComponentReadiness::Unknown);
            assert_eq!(witness.delivery_status, DeliveryTraceStatus::Unknown);
        }
        other => panic!("expected prototype witness, got {other:?}"),
    }
}

#[test]
fn daemon_serves_scaffold_observation_replies_for_all_request_families() {
    let engine = EngineId::new("prototype");

    let engine_reply = serve_one(IntrospectionRequest::EngineSnapshot(EngineSnapshotQuery {
        engine: engine.clone(),
    }));
    match engine_reply {
        IntrospectionReply::EngineSnapshot(snapshot) => {
            assert_eq!(snapshot.engine, engine);
            assert!(
                snapshot
                    .observed_components
                    .contains(&IntrospectionTarget::EngineManager)
            );
            assert!(
                snapshot
                    .observed_components
                    .contains(&IntrospectionTarget::Router)
            );
            assert!(
                snapshot
                    .observed_components
                    .contains(&IntrospectionTarget::Terminal)
            );
        }
        other => panic!("expected engine snapshot, got {other:?}"),
    }

    let component_reply = serve_one(IntrospectionRequest::ComponentSnapshot(
        ComponentSnapshotQuery {
            engine: EngineId::new("prototype"),
            target: IntrospectionTarget::Router,
        },
    ));
    match component_reply {
        IntrospectionReply::ComponentSnapshot(snapshot) => {
            assert_eq!(snapshot.target, IntrospectionTarget::Router);
            assert_eq!(snapshot.readiness, ComponentReadiness::Unknown);
        }
        other => panic!("expected component snapshot, got {other:?}"),
    }

    let delivery_reply = serve_one(IntrospectionRequest::DeliveryTrace(DeliveryTraceQuery {
        engine: EngineId::new("prototype"),
        correlation: CorrelationId::new("delivery-aab"),
    }));
    match delivery_reply {
        IntrospectionReply::DeliveryTrace(trace) => {
            assert_eq!(trace.correlation, CorrelationId::new("delivery-aab"));
            assert_eq!(trace.status, DeliveryTraceStatus::Unknown);
        }
        other => panic!("expected delivery trace, got {other:?}"),
    }
}
