//! Real-build positives are selected explicitly. Missing APIs are planned API RED.
mod tlcx22_source_build_support;
use tlcx22_source_build_support::{profiles::*, *};
use lc631_analysis::source_build_origin as b;
use std::{fs::OpenOptions, io::{Read,Write}};

#[test]
#[ignore = "central-only registered real rustc/sysroot/link profile; no fallback"]
fn tlcx22_sbo_actual_link_captures_absence_fingerprint_and_complete_process_frame() {
    assert_eq!(DOWNSTREAM_PROBE_INPUT,b"0 0\n1 2\n4294967295 1\n4294967295 0\n");
    for text in [CHECKED,WRAPPING] {
        let f=Fixture::new(text); assert!(!f.output.exists());
        f.with_input(None,|input,services,view| {
            assert_eq!(view.amount(lc631_substrate::resource::ResourceKind::Child),Ok(1));
            assert_eq!(view.amount(lc631_substrate::resource::ResourceKind::Cleanup),Ok(1));
            let plan=b::BuildPlan::given(input,view).unwrap();
            let origin=b::observe_registered_source_build(&plan,services,view)
                .unwrap_or_else(|failure|panic!("actual build failed: {:?}",failure.cause()));
            origin.with_current(&plan,services,view,|v| {
                assert_eq!(v.kind(),b::OriginKind::CompilerObservedDeclaredBuild);
                assert_eq!(v.output_state(),b::OutputState::AbsentBeforeObservedAfterLink);
                assert_eq!(v.subject_digest(),f.subject.id().digest());
                assert_eq!(v.driver_digest(),digest(DRIVER.as_bytes()));
                assert_eq!(v.entrypoint(),"checked_add");
                assert_eq!(v.frame_digest(),plan.frame_digest());
                assert_eq!(v.before_source().content_pins(),v.after_source().content_pins());
                assert_eq!(v.root_digest(),plan.root_digest());
                assert_eq!(v.program_digest(),plan.program_digest()); assert_eq!(v.plan_digest(),plan.plan_digest());
                assert_eq!(v.compiler_digest(),f.compiler.image);
                assert_eq!(v.verbose_digest(),f.compiler.verbose);
                assert_eq!(v.sysroot(),f.compiler.sysroot.as_path());
                assert_eq!(v.cwd(),f.root.as_path());
                assert_eq!(v.environment_digest(),plan.environment_digest());
                assert_eq!(v.artifact_path(),f.output.as_path());
                assert_eq!(v.artifact_digest(),file_hash(&f.output,64*1024*1024));
                let stages:Vec<_>=v.children().iter().map(|child|child.stage()).collect();
                assert_eq!(stages,[b::BuildStage::CompilerVersion,b::BuildStage::CompilerSysroot,b::BuildStage::Link]);
                for child in v.children() {
                    assert_eq!(child.exit_code(),Some(0)); assert!(child.cleanup_verified());
                    assert_eq!(child.frame_digest(),plan.frame_digest());
                    assert_eq!(child.invocation_digest(),plan.invocation_digest(child.stage()));
                    assert_eq!(child.options_digest(),plan.options_digest(child.stage()));
                    assert_eq!(child.source_revision(),view.source_revision());
                    assert_eq!(child.compiler_digest(),f.compiler.image);
                    assert_eq!(child.cwd_digest(),plan.cwd_digest());
                    assert_eq!(child.environment_digest(),plan.environment_digest());
                    assert_eq!(child.test_scope(),plan.execution_scope(child.stage()));
                    assert!(child.test_checked_epoch()>=100 && child.test_checked_epoch()<400);
                }
                // The wrapping subject is intentionally semantically imperfect;
                // provenance success cannot assert task acceptance or C1 success.
                assert_eq!(v.semantic_acceptance(),b::SemanticAcceptance::NotAssessed);
                assert_eq!(v.independence(),b::Independence::SharedDependency);
                assert_eq!(v.hermetic_closure(),b::HermeticClosure::UnobservedExternalToolchain);
                assert_eq!(v.source_goal(),None); assert_eq!(v.source_c1(),b::SourceC1::BlockedNeedsLaterGates);
            }).unwrap();
        });
    }
}

#[test]
#[ignore = "central-only stale real-native output refusal before any child"]
fn tlcx22_sbo_existing_stale_native_is_not_rebuilt_overwritten_or_retagged() {
    let f=Fixture::new(CHECKED);
    // Copy actual native compiler bytes into a NEW output slot; never overwrite.
    let mut source=std::fs::File::open(&f.compiler.path).unwrap();
    let mut output=OpenOptions::new().create_new(true).write(true).open(&f.output).unwrap();
    let mut buf=[0u8;8192];let mut total=0;
    loop{let n=source.read(&mut buf).unwrap();if n==0{break;}total+=n;assert!(total<=128*1024*1024);output.write_all(&buf[..n]).unwrap();}
    output.sync_all().unwrap(); drop(output);
    let before=file_hash(&f.output,128*1024*1024);
    f.with_input(None,|input,services,view|{
        let plan=b::BuildPlan::given(input,view).unwrap();
        let failure=b::observe_registered_source_build(&plan,services,view).err().expect("existing output must refuse");
        assert!(matches!(failure.cause(),b::BuildError::OutputAlreadyExists));
        assert_eq!(failure.observed_child_count(),0); assert!(!failure.has_origin());
    });
    assert_eq!(file_hash(&f.output,128*1024*1024),before);
}

#[test]
#[ignore = "central-only finite profile unsupported-source admission; not general Rust"]
fn tlcx22_sbo_module_include_proc_macro_and_foreign_dependency_are_visible_unsupported() {
    for subject in [
        "mod other; pub fn checked_add(a:u32,b:u32)->Option<u32>{a.checked_add(b)}\n",
        "include!(\"other.rs\");\n",
        "#[derive(MyProc)] struct X; pub fn checked_add(a:u32,b:u32)->Option<u32>{None}\n",
        "extern crate other; pub fn checked_add(a:u32,b:u32)->Option<u32>{None}\n",
    ] {
        let f=Fixture::new(subject);
        f.with_input(None,|input,services,view|{
            match b::BuildPlan::given(input,view) {
                Err(b::BuildError::Unsupported(_))=>{},
                Err(e)=>panic!("wrong admission cause {e:?}"),
                Ok(plan)=>{
                    let failure=b::observe_registered_source_build(&plan,services,view).err().expect("unsupported source");
                    assert!(matches!(failure.cause(),b::BuildError::Unsupported(_)));
                    assert_eq!(failure.observed_child_count(),0);
                }
            }
        });
        assert!(!f.output.exists());
    }
}

#[test]
#[ignore = "central-only metadata-only stage cannot mint executable origin"]
fn tlcx22_sbo_metadata_only_and_unrelated_frame_are_not_link_origins() {
    for metadata in [true,false] {
        let f=Fixture::new(CHECKED);
        f.with_input(None,|mut input,services,view|{
            if metadata {input.emission=b::Emission::MetadataOnly;}
            else { input.entrypoint="unrelated_function"; }
            match b::BuildPlan::given(input,view) {
                Err(b::BuildError::Unsupported(_)|b::BuildError::FrameMismatch)=>{},
                Err(e)=>panic!("wrong stage/frame cause {e:?}"),
                Ok(plan)=>{
                    let failure=b::observe_registered_source_build(&plan,services,view).err().expect("no link origin");
                    assert!(matches!(failure.cause(),b::BuildError::Unsupported(_)|b::BuildError::FrameMismatch));
                    assert!(!failure.has_origin());
                }
            }
        }); assert!(!f.output.exists());
    }
}
