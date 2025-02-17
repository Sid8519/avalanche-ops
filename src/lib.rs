use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, Error, ErrorKind, Write},
    path::Path,
    string::String,
};

use log::info;
use serde::{Deserialize, Serialize};

pub mod errors;

/// ref. https://doc.rust-lang.org/reference/items/modules.html
pub mod aws;

/// ref. https://doc.rust-lang.org/reference/items/modules.html
pub mod utils;
use crate::utils::{id, prefix, random, time};

/// ref. https://doc.rust-lang.org/reference/items/modules.html
pub mod avalanche;
use crate::avalanche::{
    avalanchego::{config as avalanchego_config, genesis as avalanchego_genesis},
    constants,
    coreth::config as coreth_config,
    key, node,
    subnet_evm::genesis as subnet_evm_genesis,
};

/// ref. https://doc.rust-lang.org/reference/items/modules.html
pub mod dev;

pub const DEFAULT_KEYS_TO_GENERATE: usize = 5;

/// Default machine anchor nodes size.
/// only required for custom networks
pub const DEFAULT_MACHINE_ANCHOR_NODES: u32 = 2;
pub const MIN_MACHINE_ANCHOR_NODES: u32 = 1;
pub const MAX_MACHINE_ANCHOR_NODES: u32 = 10; // TODO: allow higher number?

/// Default machine non-anchor nodes size.
pub const DEFAULT_MACHINE_NON_ANCHOR_NODES: u32 = 2;
pub const MIN_MACHINE_NON_ANCHOR_NODES: u32 = 1;
pub const MAX_MACHINE_NON_ANCHOR_NODES: u32 = 200; // TODO: allow higher number?

/// Represents network-level configuration shared among all nodes.
/// The node-level configuration is generated during each
/// bootstrap process (e.g., certificates) and not defined
/// in this cluster-level "Config".
/// At the beginning, the user is expected to provide this configuration.
/// "Clone" is for deep-copying.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Spec {
    /// User-provided ID of the cluster/test.
    /// This is NOT the avalanche node ID.
    /// This is NOT the avalanche network ID.
    #[serde(default)]
    pub id: String,

    /// AWS resources if run in AWS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aws_resources: Option<aws::Resources>,

    /// Defines how the underlying infrastructure is set up.
    /// MUST BE NON-EMPTY.
    pub machine: Machine,
    /// Install artifacts to share with remote machines.
    pub install_artifacts: InstallArtifacts,

    /// Represents the configuration for "avalanchego".
    /// Set as if run in remote machines.
    /// For instance, "config-file" must be the path valid
    /// in the remote machines.
    /// MUST BE "kebab-case" to be compatible with "avalanchego".
    pub avalanchego_config: avalanchego_config::Config,
    /// If non-empty, the JSON-encoded data are saved to a file
    /// in Path::new(&avalanchego_config.chain_config_dir).join("C").
    pub coreth_config: coreth_config::Config,
    /// If non-empty, the JSON-encoded data are saved to a file
    /// and used for "--genesis" in Path::new(&avalanchego_config.genesis).
    /// This includes "coreth_genesis::Genesis".
    /// Names after "_template" since it has not included
    /// initial stakers yet with to-be-created node IDs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avalanchego_genesis_template: Option<avalanchego_genesis::Genesis>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub subnet_evm_genesis: Option<subnet_evm_genesis::Genesis>,

    /// Generated key info with locked P-chain balance with
    /// initial stake duration in genesis.
    /// Only valid for custom networks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_seed_private_key_with_locked_p_chain_balance: Option<key::PrivateKeyInfo>,
    /// Generated key infos with immediately unlocked P-chain balance.
    /// Only pre-funded for custom networks with a custom genesis file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_seed_private_keys: Option<Vec<key::PrivateKeyInfo>>,

    /// Current all nodes. May be stale.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_nodes: Option<Vec<node::Node>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoints: Option<Endpoints>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Endpoints {
    /// Only updated after creation.
    /// READ ONLY -- DO NOT SET.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_rpc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_rpc_x: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_rpc_p: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_rpc_c: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liveness: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metamask_rpc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub websocket: Option<String>,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self::default()
    }
}

impl Endpoints {
    pub fn default() -> Self {
        Self {
            http_rpc: None,
            http_rpc_x: None,
            http_rpc_p: None,
            http_rpc_c: None,
            metrics: None,
            health: None,
            liveness: None,
            metamask_rpc: None,
            websocket: None,
        }
    }

    /// Converts to string in YAML format.
    pub fn encode_yaml(&self) -> io::Result<String> {
        match serde_yaml::to_string(&self) {
            Ok(s) => Ok(s),
            Err(e) => {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("failed to serialize DnsEndpoints to YAML {}", e),
                ));
            }
        }
    }
}

/// Defines how the underlying infrastructure is set up.
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Machine {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_nodes: Option<u32>,
    #[serde(default)]
    pub non_anchor_nodes: u32,
    #[serde(default)]
    pub instance_types: Option<Vec<String>>,
}

/// Represents artifacts for installation, to be shared with
/// remote machines. All paths are local to the caller's environment.
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
#[serde(rename_all = "snake_case")]
pub struct InstallArtifacts {
    /// "avalanched" agent binary path in the local environment.
    /// The file is uploaded to the remote storage with the path
    /// "install/avalanched" to be shared with remote machines.
    /// The file is NOT compressed when uploaded.
    #[serde(default)]
    pub avalanched_bin: String,
    /// AvalancheGo binary path in the local environment.
    /// The file is "compressed" and uploaded to remote storage
    /// to be shared with remote machines.
    ///
    ///  build
    ///    ├── avalanchego (the binary from compiling the app directory)
    ///    └── plugins
    ///        └── evm
    #[serde(default)]
    pub avalanchego_bin: String,
    /// Plugin directories in the local environment.
    /// Files (if any) are uploaded to the remote storage to be shared
    /// with remote machiens.
    #[serde(default)]
    pub plugins_dir: Option<String>,
}

/// Represents the CloudFormation stack name.
pub enum StackName {
    Ec2InstanceRole(String),
    Vpc(String),
    AsgBeaconNodes(String),
    AsgNonBeaconNodes(String),
}

impl StackName {
    pub fn encode(&self) -> String {
        match self {
            StackName::Ec2InstanceRole(id) => format!("{}-ec2-instance-role", id),
            StackName::Vpc(id) => format!("{}-vpc", id),
            StackName::AsgBeaconNodes(id) => format!("{}-asg-anchor-nodes", id),
            StackName::AsgNonBeaconNodes(id) => format!("{}-asg-non-anchor-nodes", id),
        }
    }
}

/// Defines "default-spec" option.
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct DefaultSpecOption {
    pub log_level: String,
    pub network_name: String,
    pub keys_to_generate: usize,

    pub region: String,

    pub db_backup_s3_region: String,
    pub db_backup_s3_bucket: String,
    pub db_backup_s3_key: String,

    pub nlb_acm_certificate_arn: String,

    pub install_artifacts_avalanched_bin: String,
    pub install_artifacts_avalanche_bin: String,
    pub install_artifacts_plugins_dir: String,

    pub avalanchego_log_level: String,
    pub avalanchego_whitelisted_subnets: String,
    pub avalanchego_http_tls_enabled: bool,
    pub avalanchego_state_sync_ids: String,
    pub avalanchego_state_sync_ips: String,
    pub avalanchego_profile_continuous_enabled: bool,
    pub avalanchego_profile_continuous_freq: String,
    pub avalanchego_profile_continuous_max_files: String,

    pub coreth_metrics_enabled: bool,
    pub coreth_continuous_profiler_enabled: bool,
    pub coreth_offline_pruning_enabled: bool,

    pub enable_subnet_evm: bool,

    pub disable_instance_system_logs: bool,
    pub disable_instance_system_metrics: bool,

    pub spec_file_path: String,
}

impl Spec {
    /// Creates a default Status based on the network ID.
    /// For custom networks, it generates the "keys" number of keys
    /// and pre-funds them in the genesis file path, which is
    /// included in "InstallArtifacts.genesis_draft_file_path".
    pub fn default_aws(opt: DefaultSpecOption) -> Self {
        let network_id = match constants::NETWORK_NAME_TO_NETWORK_ID.get(opt.network_name.as_str())
        {
            Some(v) => *v,
            None => avalanchego_config::DEFAULT_CUSTOM_NETWORK_ID,
        };

        let mut avalanchego_config = avalanchego_config::Config::default();
        avalanchego_config.network_id = network_id;
        avalanchego_config.log_level = Some(opt.avalanchego_log_level);
        if !avalanchego_config.is_custom_network() {
            avalanchego_config.genesis = None;
        }

        // only set values if non empty
        // otherwise, avalanchego will fail with "couldn't load node config: read .: is a directory"
        // TODO: use different certs than staking?
        if opt.avalanchego_http_tls_enabled {
            avalanchego_config.http_tls_enabled = Some(true);
            avalanchego_config.http_tls_key_file = avalanchego_config.staking_tls_key_file.clone();
            avalanchego_config.http_tls_cert_file =
                avalanchego_config.staking_tls_cert_file.clone();
        }

        if !opt.avalanchego_state_sync_ids.is_empty() {
            avalanchego_config.state_sync_ids = Some(opt.avalanchego_state_sync_ids.clone());
        };
        if !opt.avalanchego_state_sync_ips.is_empty() {
            avalanchego_config.state_sync_ips = Some(opt.avalanchego_state_sync_ips.clone());
        };
        if opt.avalanchego_profile_continuous_enabled {
            avalanchego_config.profile_continuous_enabled = Some(true);
        }
        if !opt.avalanchego_profile_continuous_freq.is_empty() {
            avalanchego_config.profile_continuous_freq =
                Some(opt.avalanchego_profile_continuous_freq.clone());
        };
        if !opt.avalanchego_profile_continuous_max_files.is_empty() {
            let profile_continuous_max_files = opt.avalanchego_profile_continuous_max_files;
            let profile_continuous_max_files = profile_continuous_max_files.parse::<u32>().unwrap();
            avalanchego_config.profile_continuous_max_files = Some(profile_continuous_max_files);
        };
        if !opt.avalanchego_whitelisted_subnets.is_empty() {
            avalanchego_config.whitelisted_subnets = Some(opt.avalanchego_whitelisted_subnets);
        };

        let network_id = avalanchego_config.network_id;
        let id = {
            if !opt.spec_file_path.is_empty() {
                let spec_file_stem = Path::new(&opt.spec_file_path).file_stem().unwrap();
                spec_file_stem.to_str().unwrap().to_string()
            } else {
                match constants::NETWORK_ID_TO_NETWORK_NAME.get(&network_id) {
                    Some(v) => id::with_time(format!("aops-{}", *v).as_str()),
                    None => id::with_time("aops-custom"),
                }
            }
        };
        let (anchor_nodes, non_anchor_nodes) =
            match constants::NETWORK_ID_TO_NETWORK_NAME.get(&network_id) {
                Some(_) => (None, DEFAULT_MACHINE_NON_ANCHOR_NODES),
                None => (
                    Some(DEFAULT_MACHINE_ANCHOR_NODES),
                    DEFAULT_MACHINE_NON_ANCHOR_NODES,
                ),
            };
        let machine = Machine {
            anchor_nodes,
            non_anchor_nodes,
            instance_types: Some(vec![
                String::from("c6a.large"),
                String::from("m6a.large"),
                String::from("m5.large"),
                String::from("c5.large"),
            ]),
        };

        let (avalanchego_genesis_template, generated_seed_keys) = {
            if avalanchego_config.is_custom_network() {
                let (g, seed_keys) =
                    avalanchego_genesis::Genesis::new(network_id, opt.keys_to_generate)
                        .expect("unexpected None genesis");
                (Some(g), seed_keys)
            } else {
                // existing network has only 1 pre-funded key "ewoq"
                let mut seed_keys: Vec<key::PrivateKeyInfo> = Vec::new();
                for i in 0..opt.keys_to_generate {
                    let k = {
                        if i < key::TEST_KEYS.len() {
                            key::TEST_KEYS[i].clone()
                        } else {
                            key::Key::generate().expect("unexpected key generate failure")
                        }
                    };
                    let info = k.to_info(network_id).expect("unexpected to_info failure");
                    seed_keys.push(info);
                }
                (None, seed_keys)
            }
        };
        let generated_seed_private_key_with_locked_p_chain_balance =
            Some(generated_seed_keys[0].clone());
        let generated_seed_private_keys = Some(generated_seed_keys[1..].to_vec());

        let subnet_evm_genesis = {
            if opt.enable_subnet_evm {
                let mut subnet_evm_seed_allocs = BTreeMap::new();
                let mut admin_addresses: Vec<String> = Vec::new();
                for key_info in generated_seed_keys.iter() {
                    subnet_evm_seed_allocs.insert(
                        String::from(prefix::strip_0x(&key_info.eth_address)),
                        subnet_evm_genesis::AllocAccount::default(),
                    );
                    admin_addresses.push(key_info.eth_address.clone());
                }
                let mut genesis = subnet_evm_genesis::Genesis::default();
                genesis.alloc = Some(subnet_evm_seed_allocs);

                let mut chain_config = subnet_evm_genesis::ChainConfig::default();
                let allow_list = subnet_evm_genesis::ContractDeployerAllowListConfig {
                    allow_list_admins: Some(admin_addresses),
                    ..subnet_evm_genesis::ContractDeployerAllowListConfig::default()
                };
                chain_config.contract_deployer_allow_list_config = Some(allow_list);
                genesis.config = Some(chain_config);

                Some(genesis)
            } else {
                None
            }
        };

        let mut aws_resources = aws::Resources {
            region: opt.region,
            s3_bucket: format!("avalanche-ops-{}-{}", time::get(6), id::system(10)), // [year][month][date]-[system host-based id]
            ..aws::Resources::default()
        };
        if !opt.db_backup_s3_region.is_empty() {
            aws_resources.db_backup_s3_region = Some(opt.db_backup_s3_region);
        }
        if !opt.db_backup_s3_bucket.is_empty() {
            aws_resources.db_backup_s3_bucket = Some(opt.db_backup_s3_bucket);
        }
        if !opt.db_backup_s3_key.is_empty() {
            aws_resources.db_backup_s3_key = Some(opt.db_backup_s3_key);
        }
        if !opt.nlb_acm_certificate_arn.is_empty() {
            aws_resources.nlb_acm_certificate_arn = Some(opt.nlb_acm_certificate_arn);
        }
        if opt.disable_instance_system_logs {
            aws_resources.instance_system_logs = Some(false);
        }
        if opt.disable_instance_system_metrics {
            aws_resources.instance_system_metrics = Some(false);
        }
        let aws_resources = Some(aws_resources);

        let mut install_artifacts = InstallArtifacts {
            avalanched_bin: opt.install_artifacts_avalanched_bin,
            avalanchego_bin: opt.install_artifacts_avalanche_bin,
            plugins_dir: None,
        };
        if !opt.install_artifacts_plugins_dir.is_empty() {
            install_artifacts.plugins_dir = Some(opt.install_artifacts_plugins_dir);
        }

        let mut coreth_config = coreth_config::Config::default();
        if opt.coreth_metrics_enabled {
            coreth_config.metrics_enabled = Some(true);
        }
        if opt.coreth_continuous_profiler_enabled {
            coreth_config.continuous_profiler_dir =
                Some(String::from(coreth_config::DEFAULT_PROFILE_DIR));
            coreth_config.continuous_profiler_frequency =
                Some(coreth_config::DEFAULT_PROFILE_FREQUENCY);
            coreth_config.continuous_profiler_max_files =
                Some(coreth_config::DEFAULT_PROFILE_MAX_FILES);
        }
        if opt.coreth_offline_pruning_enabled {
            coreth_config.offline_pruning_enabled = Some(true);
        }

        Self {
            id,

            aws_resources,
            machine,
            install_artifacts,

            avalanchego_config,
            coreth_config,
            avalanchego_genesis_template,

            subnet_evm_genesis,

            generated_seed_private_key_with_locked_p_chain_balance,
            generated_seed_private_keys,

            current_nodes: None,
            endpoints: None,
        }
    }

    /// Converts to string in YAML format.
    pub fn encode_yaml(&self) -> io::Result<String> {
        match serde_yaml::to_string(&self) {
            Ok(s) => Ok(s),
            Err(e) => {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("failed to serialize Spec to YAML {}", e),
                ));
            }
        }
    }

    /// Saves the current spec to disk
    /// and overwrites the file.
    pub fn sync(&self, file_path: &str) -> io::Result<()> {
        info!("syncing Spec to '{}'", file_path);
        let path = Path::new(file_path);
        let parent_dir = path.parent().expect("unexpected None parent");
        fs::create_dir_all(parent_dir)?;

        let ret = serde_yaml::to_vec(self);
        let d = match ret {
            Ok(d) => d,
            Err(e) => {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("failed to serialize Spec to YAML {}", e),
                ));
            }
        };
        let mut f = File::create(file_path)?;
        f.write_all(&d)?;

        Ok(())
    }

    pub fn load(file_path: &str) -> io::Result<Self> {
        info!("loading Spec from {}", file_path);

        if !Path::new(file_path).exists() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("file {} does not exists", file_path),
            ));
        }

        let f = File::open(&file_path).map_err(|e| {
            return Error::new(
                ErrorKind::Other,
                format!("failed to open {} ({})", file_path, e),
            );
        })?;
        serde_yaml::from_reader(f).map_err(|e| {
            return Error::new(ErrorKind::InvalidInput, format!("invalid YAML: {}", e));
        })
    }

    /// Validates the spec.
    pub fn validate(&self) -> io::Result<()> {
        info!("validating Spec");

        if self.id.is_empty() {
            return Err(Error::new(ErrorKind::InvalidInput, "'id' cannot be empty"));
        }

        // some AWS resources have tag limit of 32-character
        if self.id.len() > 28 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("'id' length cannot be >28 (got {})", self.id.len()),
            ));
        }

        if self.aws_resources.is_some() {
            let aws_resources = self.aws_resources.clone().unwrap();
            if aws_resources.region.is_empty() {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "'machine.region' cannot be empty",
                ));
            }
            if aws_resources.db_backup_s3_region.is_some()
                && aws_resources.db_backup_s3_bucket.is_none()
            {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!(
                        "{} missing corresponding bucket",
                        aws_resources
                            .db_backup_s3_bucket
                            .expect("unexpected aws_resources.db_backup_s3_bucket")
                    ),
                ));
            }
            if aws_resources.db_backup_s3_bucket.is_some()
                && aws_resources.db_backup_s3_key.is_none()
            {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!(
                        "{} missing corresponding key",
                        aws_resources
                            .db_backup_s3_bucket
                            .expect("unexpected aws_resources.db_backup_s3_bucket")
                    ),
                ));
            }
            if aws_resources.db_backup_s3_bucket.is_some()
                && aws_resources.db_backup_s3_region.is_none()
            {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!(
                        "{} missing corresponding region",
                        aws_resources
                            .db_backup_s3_bucket
                            .expect("unexpected aws_resources.db_backup_s3_bucket")
                    ),
                ));
            }
        }

        if self.machine.non_anchor_nodes < MIN_MACHINE_NON_ANCHOR_NODES {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "'machine.non_anchor_nodes' {} <minimum {}",
                    self.machine.non_anchor_nodes, MIN_MACHINE_NON_ANCHOR_NODES
                ),
            ));
        }
        if self.machine.non_anchor_nodes > MAX_MACHINE_NON_ANCHOR_NODES {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "'machine.non_anchor_nodes' {} >maximum {}",
                    self.machine.non_anchor_nodes, MAX_MACHINE_NON_ANCHOR_NODES
                ),
            ));
        }

        if !Path::new(&self.install_artifacts.avalanched_bin).exists() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "avalanched_bin {} does not exist",
                    self.install_artifacts.avalanched_bin
                ),
            ));
        }
        if !Path::new(&self.install_artifacts.avalanchego_bin).exists() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "avalanchego_bin {} does not exist",
                    self.install_artifacts.avalanchego_bin
                ),
            ));
        }
        if self.install_artifacts.plugins_dir.is_some()
            && !Path::new(
                &self
                    .install_artifacts
                    .plugins_dir
                    .clone()
                    .expect("unexpected None install_artifacts.plugins_dir"),
            )
            .exists()
        {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "plugins_dir {} does not exist",
                    self.install_artifacts
                        .plugins_dir
                        .clone()
                        .expect("unexpected None install_artifacts.plugins_dir")
                ),
            ));
        }

        if !self.avalanchego_config.is_custom_network() {
            if self.avalanchego_genesis_template.is_some() {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!(
                        "cannot specify 'avalanchego_genesis_template' for network_id {:?}",
                        self.avalanchego_config.network_id
                    ),
                ));
            }
            if self.machine.anchor_nodes.unwrap_or(0) > 0 {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!(
                        "cannot specify non-zero 'machine.anchor_nodes' for network_id {:?}",
                        self.avalanchego_config.network_id
                    ),
                ));
            }
        } else {
            if self.avalanchego_genesis_template.is_none() {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!(
                        "must specify 'avalanchego_genesis_template' for network_id {:?}",
                        self.avalanchego_config.network_id
                    ),
                ));
            }
            if self.machine.anchor_nodes.unwrap_or(0) == 0 {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "cannot specify 0 for 'machine.anchor_nodes' for custom network",
                ));
            }
            if self.machine.anchor_nodes.unwrap_or(0) < MIN_MACHINE_ANCHOR_NODES {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!(
                        "'machine.anchor_nodes' {} below min {}",
                        self.machine.anchor_nodes.unwrap_or(0),
                        MIN_MACHINE_ANCHOR_NODES
                    ),
                ));
            }
            if self.machine.anchor_nodes.unwrap_or(0) > MAX_MACHINE_ANCHOR_NODES {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!(
                        "'machine.anchor_nodes' {} exceeds limit {}",
                        self.machine.anchor_nodes.unwrap_or(0),
                        MAX_MACHINE_ANCHOR_NODES
                    ),
                ));
            }
        }

        Ok(())
    }
}

#[test]
fn test_spec() {
    use std::fs;
    let _ = env_logger::builder().is_test(true).try_init();

    let mut f = tempfile::NamedTempFile::new().unwrap();
    let ret = f.write_all(&vec![0]);
    assert!(ret.is_ok());
    let avalanched_bin = f.path().to_str().unwrap();

    let mut f = tempfile::NamedTempFile::new().unwrap();
    let ret = f.write_all(&vec![0]);
    assert!(ret.is_ok());
    let avalanchego_bin = f.path().to_str().unwrap();

    let tmp_dir = tempfile::tempdir().unwrap();
    let plugin_path = tmp_dir.path().join(random::string(10));
    let mut f = File::create(&plugin_path).unwrap();
    let ret = f.write_all(&vec![0]);
    assert!(ret.is_ok());
    let plugins_dir = tmp_dir.path().as_os_str().to_str().unwrap();

    // test just to see how "read_dir" works in Rust
    for entry in fs::read_dir(plugins_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        info!("read_dir: {:?}", path);
    }

    let id = random::string(10);
    let bucket = format!("test-{}", time::get(8));

    let contents = format!(
        r#"

id: {}

aws_resources:
  region: us-west-2
  s3_bucket: {}
  instance_system_logs: true
  instance_system_metrics: true

machine:
  non_anchor_nodes: 20
  instance_types:
  - m5.large
  - c5.large
  - r5.large
  - t3.large

install_artifacts:
  avalanched_bin: {}
  avalanchego_bin: {}
  plugins_dir: {}

avalanchego_config:
  config-file: /etc/avalanche.config.json
  network-id: 1
  db-type: leveldb
  db-dir: /avalanche-data
  log-dir: /var/log/avalanche
  log-level: INFO
  http-port: 9650
  http-host: 0.0.0.0
  http-tls-enabled: false
  staking-enabled: true
  staking-port: 9651
  staking-tls-key-file: "/etc/pki/tls/certs/avalanched.pki.key"
  staking-tls-cert-file: "/etc/pki/tls/certs/avalanched.pki.crt"
  snow-sample-size: 20
  snow-quorum-size: 15
  index-enabled: false
  index-allow-incomplete: false
  api-admin-enabled: true
  api-info-enabled: true
  api-keystore-enabled: true
  api-metrics-enabled: true
  api-health-enabled: true
  api-ipcs-enabled: true
  chain-config-dir: /etc/avalanche/configs/chains
  subnet-config-dir: /etc/avalanche/configs/subnets
  profile-dir: /var/log/avalanche-profile/avalanche

coreth_config:
  coreth-admin-api-enabled: true
  metrics-enabled: true
  log-level: "info"


"#,
        id, bucket, avalanched_bin, avalanchego_bin, plugins_dir,
    );
    let mut f = tempfile::NamedTempFile::new().unwrap();
    let ret = f.write_all(contents.as_bytes());
    assert!(ret.is_ok());
    let config_path = f.path().to_str().unwrap();

    let ret = Spec::load(config_path);
    assert!(ret.is_ok());
    let cfg = ret.unwrap();

    let ret = cfg.sync(config_path);
    assert!(ret.is_ok());

    let mut avalanchego_config = avalanchego_config::Config::default();
    avalanchego_config.genesis = None;
    avalanchego_config.network_id = 1;

    let orig = Spec {
        id: id.clone(),

        aws_resources: Some(aws::Resources {
            region: String::from("us-west-2"),
            s3_bucket: bucket.clone(),
            ..aws::Resources::default()
        }),

        machine: Machine {
            anchor_nodes: None,
            non_anchor_nodes: 20,
            instance_types: Some(vec![
                String::from("m5.large"),
                String::from("c5.large"),
                String::from("r5.large"),
                String::from("t3.large"),
            ]),
        },

        install_artifacts: InstallArtifacts {
            avalanched_bin: avalanched_bin.to_string(),
            avalanchego_bin: avalanchego_bin.to_string(),
            plugins_dir: Some(plugins_dir.to_string()),
        },

        avalanchego_config,
        coreth_config: coreth_config::Config::default(),
        avalanchego_genesis_template: None,

        subnet_evm_genesis: None,

        generated_seed_private_key_with_locked_p_chain_balance: None,
        generated_seed_private_keys: None,
        current_nodes: None,
        endpoints: None,
    };

    assert_eq!(cfg, orig);
    cfg.validate().expect("unexpected validate failure");
    orig.validate().expect("unexpected validate failure");

    // manually check to make sure the serde deserializer works
    assert_eq!(cfg.id, id);

    let aws_resources = cfg.aws_resources.unwrap();
    assert_eq!(aws_resources.region, "us-west-2");
    assert_eq!(aws_resources.s3_bucket, bucket);

    assert_eq!(cfg.install_artifacts.avalanched_bin, avalanched_bin);
    assert_eq!(cfg.install_artifacts.avalanchego_bin, avalanchego_bin);
    assert_eq!(
        cfg.install_artifacts
            .plugins_dir
            .unwrap_or(String::from("")),
        plugins_dir.to_string()
    );

    assert!(cfg.machine.anchor_nodes.is_none());
    assert_eq!(cfg.machine.non_anchor_nodes, 20);
    assert!(cfg.machine.instance_types.is_some());
    let instance_types = cfg.machine.instance_types.unwrap();
    assert_eq!(instance_types[0], "m5.large");
    assert_eq!(instance_types[1], "c5.large");
    assert_eq!(instance_types[2], "r5.large");
    assert_eq!(instance_types[3], "t3.large");

    assert_eq!(cfg.avalanchego_config.clone().network_id, 1);
    assert_eq!(
        cfg.avalanchego_config
            .clone()
            .config_file
            .unwrap_or("".to_string()),
        avalanchego_config::DEFAULT_CONFIG_FILE_PATH,
    );
    assert_eq!(
        cfg.avalanchego_config.clone().snow_sample_size.unwrap_or(0),
        20
    );
    assert_eq!(
        cfg.avalanchego_config.clone().snow_quorum_size.unwrap_or(0),
        15
    );
    assert_eq!(
        cfg.avalanchego_config.clone().http_port,
        avalanchego_config::DEFAULT_HTTP_PORT,
    );
    assert_eq!(
        cfg.avalanchego_config.clone().staking_port,
        avalanchego_config::DEFAULT_STAKING_PORT,
    );
    assert_eq!(
        cfg.avalanchego_config.clone().db_dir,
        avalanchego_config::DEFAULT_DB_DIR,
    );
}

/// Represents the S3/storage key path.
/// MUST be kept in sync with "src/aws/cfn-templates/avalanche-node/ec2_instance_role.yaml".
pub enum StorageNamespace {
    ConfigFile(String),
    DevMachineConfigFile(String),
    Ec2AccessKeyCompressedEncrypted(String),

    /// Valid genesis file with initial stakers.
    /// Only updated after anchor nodes become active.
    GenesisFile(String),

    AvalanchedBin(String),
    AvalancheBinCompressed(String),
    PluginsDir(String),

    PkiKeyDir(String),

    /// before db downloads
    DiscoverProvisioningAnchorNodesDir(String),
    DiscoverProvisioningAnchorNode(String, node::Node),
    DiscoverProvisioningNonAnchorNodesDir(String),
    DiscoverProvisioningNonAnchorNode(String, node::Node),

    DiscoverBootstrappingAnchorNodesDir(String),
    DiscoverBootstrappingAnchorNode(String, node::Node),

    DiscoverReadyAnchorNodesDir(String),
    DiscoverReadyAnchorNode(String, node::Node),
    DiscoverReadyNonAnchorNodesDir(String),
    DiscoverReadyNonAnchorNode(String, node::Node),

    BackupsDir(String),

    /// If this "event" file has been modified for the last x-min,
    /// avalanched triggers updates events based on the install artifacts
    /// in "EventsUpdateArtifactsInstallDir"
    EventsUpdateArtifactsEvent(String),
    EventsUpdateArtifactsInstallDirAvalancheBinCompressed(String),
    EventsUpdateArtifactsInstallDirPluginsDir(String),
}

impl StorageNamespace {
    pub fn encode(&self) -> String {
        match self {
            StorageNamespace::ConfigFile(id) => format!("{}/avalanche-ops.config.yaml", id),
            StorageNamespace::DevMachineConfigFile(id) => format!("{}/dev-machine.config.yaml", id),
            StorageNamespace::Ec2AccessKeyCompressedEncrypted(id) => {
                format!("{}/ec2-access-key.zstd.seal_aes_256.encrypted", id)
            }

            StorageNamespace::GenesisFile(id) => format!("{}/genesis.json", id),

            StorageNamespace::AvalanchedBin(id) => format!("{}/install/avalanched", id),
            StorageNamespace::AvalancheBinCompressed(id) => {
                format!("{}/install/avalanche.zstd", id)
            }
            StorageNamespace::PluginsDir(id) => format!("{}/install/plugins", id),

            StorageNamespace::PkiKeyDir(id) => {
                format!("{}/pki", id)
            }

            StorageNamespace::DiscoverProvisioningAnchorNodesDir(id) => {
                format!("{}/discover/provisioning-non-anchor-nodes", id)
            }
            StorageNamespace::DiscoverProvisioningAnchorNode(id, node) => {
                let compressed_id = node.compress_base58().unwrap();
                format!(
                    "{}/discover/provisioning-non-anchor-nodes/{}_{}.yaml",
                    id, node.machine_id, compressed_id
                )
            }
            StorageNamespace::DiscoverProvisioningNonAnchorNodesDir(id) => {
                format!("{}/discover/provisioning-non-anchor-nodes", id)
            }
            StorageNamespace::DiscoverProvisioningNonAnchorNode(id, node) => {
                let compressed_id = node.compress_base58().unwrap();
                format!(
                    "{}/discover/provisioning-non-anchor-nodes/{}_{}.yaml",
                    id, node.machine_id, compressed_id
                )
            }

            StorageNamespace::DiscoverBootstrappingAnchorNodesDir(id) => {
                format!("{}/discover/bootstrapping-anchor-nodes", id)
            }
            StorageNamespace::DiscoverBootstrappingAnchorNode(id, node) => {
                let compressed_id = node.compress_base58().unwrap();
                format!(
                    "{}/discover/bootstrapping-anchor-nodes/{}_{}.yaml",
                    id, node.machine_id, compressed_id
                )
            }

            StorageNamespace::DiscoverReadyAnchorNodesDir(id) => {
                format!("{}/discover/ready-anchor-nodes", id)
            }
            StorageNamespace::DiscoverReadyAnchorNode(id, node) => {
                let compressed_id = node.compress_base58().unwrap();
                format!(
                    "{}/discover/ready-anchor-nodes/{}_{}.yaml",
                    id, node.machine_id, compressed_id
                )
            }
            StorageNamespace::DiscoverReadyNonAnchorNodesDir(id) => {
                format!("{}/discover/ready-non-anchor-nodes", id)
            }
            StorageNamespace::DiscoverReadyNonAnchorNode(id, node) => {
                let compressed_id = node.compress_base58().unwrap();
                format!(
                    "{}/discover/ready-non-anchor-nodes/{}_{}.yaml",
                    id, node.machine_id, compressed_id
                )
            }

            StorageNamespace::BackupsDir(id) => {
                format!("{}/backups", id)
            }

            StorageNamespace::EventsUpdateArtifactsEvent(id) => {
                format!("{}/events/update-artifacts/event", id)
            }
            StorageNamespace::EventsUpdateArtifactsInstallDirAvalancheBinCompressed(id) => {
                format!("{}/events/update-artifacts/install/avalanche.zstd", id)
            }
            StorageNamespace::EventsUpdateArtifactsInstallDirPluginsDir(id) => {
                format!("{}/events/update-artifacts/install/plugins", id)
            }
        }
    }

    pub fn parse_node_from_path(storage_path: &str) -> io::Result<node::Node> {
        let p = Path::new(storage_path);
        let file_name = match p.file_name() {
            Some(v) => v,
            None => {
                return Err(Error::new(
                    ErrorKind::Other,
                    String::from("failed Path.file_name (None)"),
                ));
            }
        };
        let file_name = file_name.to_str().unwrap();
        let splits: Vec<&str> = file_name.split('_').collect();
        if splits.len() != 2 {
            return Err(Error::new(
                ErrorKind::Other,
                format!(
                    "file name {} of storage_path {} expected two splits for '_' (got {})",
                    file_name,
                    storage_path,
                    splits.len(),
                ),
            ));
        }

        let compressed_id = splits[1];
        match node::Node::decompress_base58(compressed_id.replace(".yaml", "")) {
            Ok(node) => Ok(node),
            Err(e) => {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("failed node::Node::decompress_base64 {}", e),
                ));
            }
        }
    }
}

#[test]
fn test_storage_path() {
    use crate::utils::random;
    let _ = env_logger::builder().is_test(true).try_init();

    let id = random::string(10);
    let instance_id = random::string(5);
    let node_id = "NodeID-7Xhw2mDxuDS44j42TCB6U5579esbSt3Lg";
    let node_ip = "1.2.3.4";

    let node = node::Node::new(
        node::Kind::NonAnchor,
        &instance_id,
        node_id,
        node_ip,
        "http",
        9650,
    );
    let p = StorageNamespace::DiscoverReadyNonAnchorNode(
        id,
        node::Node {
            kind: String::from("non-anchor"),
            machine_id: instance_id.clone(),
            node_id: node_id.to_string(),
            public_ip: node_ip.to_string(),
            http_endpoint: format!("http://{}:9650", node_ip),
        },
    );
    let storage_path = p.encode();
    info!("KeyPath: {}", storage_path);

    let node_parsed = StorageNamespace::parse_node_from_path(&storage_path).unwrap();
    assert_eq!(node, node_parsed);
}
