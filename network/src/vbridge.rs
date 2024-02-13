use anyhow::{anyhow, Result};
use bytes::BytesMut;
use etherparse::{EtherType, Ethernet2Header, IpNumber, Ipv4Header, TcpHeader};
use log::{debug, trace, warn};
use smoltcp::wire::EthernetAddress;
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};
use tokio::sync::broadcast::{
    channel as broadcast_channel, Receiver as BroadcastReceiver, Sender as BroadcastSender,
};
use tokio::{
    select,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Mutex,
    },
    task::JoinHandle,
};

const TO_BRIDGE_QUEUE_LEN: usize = 50;
const FROM_BRIDGE_QUEUE_LEN: usize = 50;
const BROADCAST_QUEUE_LEN: usize = 50;

#[derive(Debug)]
struct BridgeMember {
    pub from_bridge_sender: Sender<BytesMut>,
}

pub struct BridgeJoinHandle {
    pub to_bridge_sender: Sender<BytesMut>,
    pub from_bridge_receiver: Receiver<BytesMut>,
    pub from_broadcast_receiver: BroadcastReceiver<BytesMut>,
}

type VirtualBridgeMemberMap = Arc<Mutex<HashMap<EthernetAddress, BridgeMember>>>;

#[derive(Clone)]
pub struct VirtualBridge {
    members: VirtualBridgeMemberMap,
    to_bridge_sender: Sender<BytesMut>,
    from_broadcast_sender: BroadcastSender<BytesMut>,
    _task: Arc<JoinHandle<()>>,
}

enum VirtualBridgeSelect {
    BroadcastSent(Option<BytesMut>),
    PacketReceived(Option<BytesMut>),
}

impl VirtualBridge {
    pub fn new() -> Result<VirtualBridge> {
        let (to_bridge_sender, to_bridge_receiver) = channel::<BytesMut>(TO_BRIDGE_QUEUE_LEN);
        let (from_broadcast_sender, from_broadcast_receiver) =
            broadcast_channel(BROADCAST_QUEUE_LEN);

        let members = Arc::new(Mutex::new(HashMap::new()));
        let handle = {
            let members = members.clone();
            let broadcast_rx_sender = from_broadcast_sender.clone();
            tokio::task::spawn(async move {
                if let Err(error) = VirtualBridge::process(
                    members,
                    to_bridge_receiver,
                    broadcast_rx_sender,
                    from_broadcast_receiver,
                )
                .await
                {
                    warn!("virtual bridge processing task failed: {}", error);
                }
            })
        };

        Ok(VirtualBridge {
            to_bridge_sender,
            members,
            from_broadcast_sender,
            _task: Arc::new(handle),
        })
    }

    pub async fn join(&self, mac: EthernetAddress) -> Result<BridgeJoinHandle> {
        let (from_bridge_sender, from_bridge_receiver) = channel::<BytesMut>(FROM_BRIDGE_QUEUE_LEN);
        let member = BridgeMember { from_bridge_sender };

        match self.members.lock().await.entry(mac) {
            Entry::Occupied(_) => {
                return Err(anyhow!("virtual bridge member {} already exists", mac));
            }
            Entry::Vacant(entry) => {
                entry.insert(member);
            }
        };
        debug!("virtual bridge member {} has joined", mac);
        Ok(BridgeJoinHandle {
            from_bridge_receiver,
            from_broadcast_receiver: self.from_broadcast_sender.subscribe(),
            to_bridge_sender: self.to_bridge_sender.clone(),
        })
    }

    async fn process(
        members: VirtualBridgeMemberMap,
        mut to_bridge_receiver: Receiver<BytesMut>,
        broadcast_rx_sender: BroadcastSender<BytesMut>,
        mut from_broadcast_receiver: BroadcastReceiver<BytesMut>,
    ) -> Result<()> {
        loop {
            let selection = select! {
                biased;
                x = from_broadcast_receiver.recv() => VirtualBridgeSelect::BroadcastSent(x.ok()),
                x = to_bridge_receiver.recv() => VirtualBridgeSelect::PacketReceived(x),
            };

            match selection {
                VirtualBridgeSelect::PacketReceived(Some(mut packet)) => {
                    let (header, payload) = match Ethernet2Header::from_slice(&packet) {
                        Ok(data) => data,
                        Err(error) => {
                            debug!("virtual bridge failed to parse ethernet header: {}", error);
                            continue;
                        }
                    };

                    if header.ether_type == EtherType::IPV4 {
                        let (ipv4, payload) = Ipv4Header::from_slice(payload)?;

                        // recalculate TCP checksums when routing packets.
                        // the xen network backend / frontend drivers for linux
                        // are very stupid and do not calculate these properly
                        // despite all best attempts at making it do so.
                        if ipv4.protocol == IpNumber::TCP {
                            let (mut tcp, payload) = TcpHeader::from_slice(payload)?;
                            tcp.checksum = tcp.calc_checksum_ipv4(&ipv4, payload)?;
                            let tcp_header_offset = Ethernet2Header::LEN + ipv4.header_len();
                            let tcp_header_bytes = tcp.to_bytes();
                            for (i, b) in tcp_header_bytes.iter().enumerate() {
                                packet[tcp_header_offset + i] = *b;
                            }
                        }
                    }

                    let destination = EthernetAddress(header.destination);
                    if destination.is_multicast() {
                        trace!(
                            "broadcasting bridge packet from {}",
                            EthernetAddress(header.source)
                        );
                        broadcast_rx_sender.send(packet)?;
                        continue;
                    }
                    match members.lock().await.get(&destination) {
                        Some(member) => {
                            member.from_bridge_sender.try_send(packet)?;
                            trace!(
                                "sending bridged packet from {} to {}",
                                EthernetAddress(header.source),
                                EthernetAddress(header.destination)
                            );
                        }
                        None => {
                            trace!("no bridge member with address: {}", destination);
                        }
                    }
                }

                VirtualBridgeSelect::PacketReceived(None) => break,
                VirtualBridgeSelect::BroadcastSent(_) => {}
            }
        }
        Ok(())
    }
}
