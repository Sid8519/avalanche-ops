use std::{
    thread,
    time::{self, Duration},
};

use aws_sdk_cloudformation::model::{Capability, OnFailure, Parameter, StackStatus, Tag};
use log::info;
use rust_embed::RustEmbed;

extern crate avalanche_ops;
use avalanche_ops::{
    aws::{self, cloudformation},
    utils::id,
};

/// cargo run --example aws_cloudformation_ec2_instance_role
fn main() {
    // ref. https://github.com/env-logger-rs/env_logger/issues/47
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    macro_rules! ab {
        ($e:expr) => {
            tokio_test::block_on($e)
        };
    }

    info!("creating AWS CloudFormation resources!");

    #[derive(RustEmbed)]
    #[folder = "src/aws/cfn-templates/avalanche-node/"]
    #[prefix = "src/aws/cfn-templates/avalanche-node/"]
    struct Asset;

    let ec2_instance_role_yaml =
        Asset::get("src/aws/cfn-templates/avalanche-node/ec2_instance_role.yaml").unwrap();
    let ret = std::str::from_utf8(ec2_instance_role_yaml.data.as_ref());
    let template_body = ret.unwrap();
    info!("{:?}", template_body);

    let ret = ab!(aws::load_config(None));
    let shared_config = ret.unwrap();
    let cloudformation_manager = cloudformation::Manager::new(&shared_config);

    let stack_name = id::with_time("test");

    // error should be ignored if it does not exist
    let ret = ab!(cloudformation_manager.delete_stack(&stack_name));
    assert!(ret.is_ok());

    let ret = ab!(cloudformation_manager.create_stack(
        &stack_name,
        Some(vec![Capability::CapabilityNamedIam]),
        OnFailure::Delete,
        template_body,
        Some(Vec::from([
            Tag::builder().key("KIND").value("avalanche-ops").build(),
            Tag::builder().key("a").value("b").build()
        ])),
        Some(Vec::from([
            Parameter::builder()
                .parameter_key("Id")
                .parameter_value(id::with_time("id"))
                .build(),
            Parameter::builder()
                .parameter_key("KmsCmkArn")
                .parameter_value("arn:aws:kms:us-west-2:123:key/456")
                .build(),
            Parameter::builder()
                .parameter_key("S3BucketName")
                .parameter_value(id::with_time("id"))
                .build(),
            Parameter::builder()
                .parameter_key("S3BucketDbBackupName")
                .parameter_value(id::with_time("id"))
                .build(),
        ])),
    ));
    let stack = ret.unwrap();
    assert_eq!(stack.name, stack_name);
    assert_eq!(stack.status, StackStatus::CreateInProgress);
    let ret = ab!(cloudformation_manager.poll_stack(
        &stack_name,
        StackStatus::CreateComplete,
        Duration::from_secs(300),
        Duration::from_secs(20),
    ));
    let stack = ret.unwrap();
    assert_eq!(stack.name, stack_name);
    assert_eq!(stack.status, StackStatus::CreateComplete);
    let outputs = stack.outputs.unwrap();
    for o in outputs {
        info!(
            "output key=[{}], value=[{}]",
            o.output_key.unwrap(),
            o.output_value.unwrap()
        )
    }

    thread::sleep(time::Duration::from_secs(5));

    let ret = ab!(cloudformation_manager.delete_stack(&stack_name));
    assert!(ret.is_ok());
    let ret = ab!(cloudformation_manager.poll_stack(
        &stack_name,
        StackStatus::DeleteComplete,
        Duration::from_secs(300),
        Duration::from_secs(20),
    ));
    let stack = ret.unwrap();
    assert_eq!(stack.name, stack_name);
    assert_eq!(stack.status, StackStatus::DeleteComplete);
}
