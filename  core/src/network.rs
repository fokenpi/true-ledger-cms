use libp2p::{identity, noise, tcp, yamux, Swarm, Transport};
use std::error::Error;

pub async fn start_p2p_node() -> Result<(), Box<dyn Error>> {
    let local_key = identity::Keypair::generate_ed25519();
    let noise_keys = noise::Keypair::<noise::X25519>::new()
        .into_authentic(&local_key)?;

    let transport = tcp::tokio::Transport::default()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(yamux::YamuxConfig::default())
        .boxed();

    println!("✅ P2P node ready");
    Ok(())
}