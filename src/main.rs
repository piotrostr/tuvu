use crossbeam_channel::unbounded;
use serde::{Deserialize, Serialize};
use solana_core::tpu::DEFAULT_TPU_COALESCE;
use solana_frozen_abi_macro::{frozen_abi, AbiEnumVisitor, AbiExample};
use solana_gossip::crds_gossip_pull::CrdsFilter;
use solana_gossip::crds_value::CrdsValue;
use solana_gossip::legacy_contact_info::LegacyContactInfo as ContactInfo;
use solana_gossip::ping_pong::Pong;
use solana_gossip::{
    cluster_info::{ClusterInfo, Node},
    gossip_service::GossipService,
};
use solana_perf::{packet::PacketBatchRecycler, recycler::Recycler};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_sdk::{signature::read_keypair_file, signer::Signer, timing::timestamp};
use solana_streamer::{
    socket::SocketAddrSpace,
    streamer::{self, StreamerReceiveStats},
};
use std::{
    io::Read,
    net::SocketAddr,
    sync::{atomic::AtomicBool, Arc},
    thread::sleep,
    time::Duration,
};

pub fn get_cluster_entrypoints() -> Vec<String> {
    let entrypoints_response = std::fs::read_to_string("entrypoints.json").unwrap();
    let entrypoints: serde_json::Value = serde_json::from_str(&entrypoints_response).unwrap();
    let valid_entrypoints = entrypoints["result"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entrypoint| {
            entrypoint["version"]
                .as_str()
                .unwrap_or_default()
                .starts_with('1')
                && entrypoint["rpc"].as_str().is_some()
                && entrypoint["gossip"].as_str().unwrap().contains(":8001")
                && entrypoint["shredVersion"].as_u64().unwrap() == 50093
        })
        .map(|entrypoint| entrypoint["gossip"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    println!("total entrypoints: {}", valid_entrypoints.len());
    valid_entrypoints
}

const GOSSIP_PING_TOKEN_SIZE: usize = 32;
pub type Ping = solana_gossip::ping_pong::Ping<[u8; GOSSIP_PING_TOKEN_SIZE]>;
#[derive(Clone, Debug, Default, Deserialize, Serialize, AbiExample)]
pub struct PruneData {
    /// Pubkey of the node that sent this prune data
    pubkey: Pubkey,
    /// Pubkeys of nodes that should be pruned
    prunes: Vec<Pubkey>,
    /// Signature of this Prune Message
    signature: Signature,
    /// The Pubkey of the intended node/destination for this message
    destination: Pubkey,
    /// Wallclock of the node that generated this message
    wallclock: u64,
}

// TODO These messages should go through the gpu pipeline for spam filtering
#[frozen_abi(digest = "ogEqvffeEkPpojAaSiUbCv2HdJcdXDQ1ykgYyvKvLo2")]
#[derive(Serialize, Deserialize, Debug, AbiEnumVisitor, AbiExample)]
#[allow(clippy::large_enum_variant)]
pub enum Protocol {
    /// Gossip protocol messages
    PullRequest(CrdsFilter, CrdsValue),
    PullResponse(Pubkey, Vec<CrdsValue>),
    PushMessage(Pubkey, Vec<CrdsValue>),
    // TODO: Remove the redundant outer pubkey here,
    // and use the inner PruneData.pubkey instead.
    PruneMessage(Pubkey, PruneData),
    PingMessage(Ping),
    PongMessage(Pong),
    // Update count_packets_received if new variants are added here.
}

pub struct Args {
    dynamic_port_range: String,
    bind_address: String,
}

fn main() {
    tracing_subscriber::fmt::init();

    // random node found on extrnode rpc list -- nmap to find the gossip port
    let socket_addr_space = SocketAddrSpace::Unspecified;
    let keypair_path = "../keys/validator-keypair.json";
    let keypair = read_keypair_file(keypair_path).unwrap();

    let identity_keypair = Arc::new(keypair);
    let shred_version = 50093; // !
    let exit = Arc::new(AtomicBool::new(false));

    let args = Args {
        dynamic_port_range: "1024-65535".to_string(),
        bind_address: "0.0.0.0".to_string(),
    };
    let gossip_host = "127.0.0.1";
    let gossip_addr = SocketAddr::new(gossip_host.parse().unwrap(), 8001);
    let dynamic_port_range = solana_net_utils::parse_port_range(args.dynamic_port_range.as_str())
        .expect("invalid dynamic_port_range");

    // IP Address        |Age(ms)| Node identifier                              | Version |Gossip|TPUvote| TPU  |TPUfwd| TVU  |TVUfwd|Repair|ServeR|ShredVer
    // 127.0.0.1       me|  2369 | 4qkukvpiYets78w3pmq5LUm5S8aww6JR1bJwqRenHJD5 | 1.16.0  | 8001 |  none | none | none | 8003 | 8002 | none | none | 56177
    let mut node = Node::new_with_external_ip(
        &identity_keypair.pubkey(),
        &gossip_addr,
        dynamic_port_range,
        args.bind_address.parse().unwrap(),
        None, // default tpu address
        None, // tpu forward address
    );
    node.sockets.ip_echo = None; // dont be an entrypoint
    node.info.set_wallclock(timestamp());
    node.info.set_shred_version(shred_version);

    // node.info.remove_tvu();
    // node.info.remove_tvu_forwards();

    // just gossip bc we got no stake
    node.info.remove_tpu();
    //node.info.remove_tpu_vote();
    node.info.remove_tpu_forwards();
    node.info.remove_serve_repair();
    // node.info.remove_repair(); // ?is this ok

    // Validator::print_node_info(&node);
    //
    let _cluster_entrypoints = get_cluster_entrypoints();

    let mut cluster_entrypoints = vec![
        gossip_addr,
        // "147.75.80.133:8001".parse().unwrap(),
        // "145.40.97.55:8001".parse().unwrap(),
        // "74.118.139.147:8001".parse().unwrap(),
        // "204.16.242.103:8001".parse().unwrap(),
    ];
    for entrypoint in _cluster_entrypoints.iter().take(5) {
        match entrypoint.parse::<SocketAddr>() {
            Ok(entrypoint) => cluster_entrypoints.push(entrypoint),
            Err(err) => {
                eprintln!("failed to parse entrypoint: {entrypoint:?} {err:?}");
            }
        }
    }
    let cluster_entrypoints = cluster_entrypoints
        .iter()
        .map(ContactInfo::new_gossip_entry_point)
        .collect::<Vec<_>>();

    // setup cluster info
    let cluster_info = ClusterInfo::new(
        node.info.clone(),
        identity_keypair.clone(),
        socket_addr_space,
    );
    cluster_info.set_entrypoints(cluster_entrypoints);
    let cluster_info = Arc::new(cluster_info);

    let _gossip_service = GossipService::new(
        &cluster_info,
        None,
        node.sockets.gossip,
        None,
        true,
        None,
        exit.clone(),
    );
    // gossip_service.join().unwrap();

    // tvu recieving shreds
    let (packet_sender, packet_receiver) = unbounded();

    let recycler: PacketBatchRecycler = Recycler::warmed(1000, 1024);
    let coalesce = DEFAULT_TPU_COALESCE;

    let _tvu_streamers: Vec<_> = node
        .sockets
        .tvu
        .into_iter()
        .map(|s| {
            streamer::receiver(
                Arc::new(s),
                exit.clone(),
                packet_sender.clone(),
                recycler.clone(),
                Arc::new(StreamerReceiveStats::new("tvu_reciever")),
                coalesce,
                true,
                None,
                false,
            )
        })
        .collect();

    loop {
        if let Ok(packet) = packet_receiver.try_recv() {
            println!("received tvu packet: {packet:?}");
        }

        let n_peers = cluster_info.all_peers().len();
        println!("n peers: {n_peers:?}...");

        println!("sleeping...");
        sleep(Duration::from_secs(1));
    }
}
