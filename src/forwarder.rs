use std::{
	io::{Read, Write},
	net::{SocketAddr, TcpListener as StdTcpListener, ToSocketAddrs},
	thread::{self, JoinHandle},
};

use futures::{sync::oneshot, Future, Stream};
use tokio_core::{
	net::{TcpListener, TcpStream},
	reactor::{Core, Handle},
};
use tokio_io::{io::copy, AsyncWrite};

use crate::{
	async_stream::AsyncTcpStream,
	error::{
		Error,
		Error::{AddressParse, Argument, Bind, ChannelCancelled, FamilyMix, Listen, ProxyHeaderFailed, ThreadStart},
	},
};

/// Every Thread instance represents one usage of the --forward argument, which forwards one bound port to a target
/// haproxy v1 enabled server.
pub(super) struct Thread {
	bind: SocketAddr,
	target: SocketAddr,
}

impl Thread {
	/// To create a Thread instance, the command line argument needs to be passed
	pub(super) fn new(arg: String) -> Result<Self, Error> {
		// We main a format: address->address
		let split: Vec<&str> = arg.split('@').collect();
		if split.len() != 2 {
			return Err(Argument { input: arg });
		}

		// Resolve both the bind and target hosts
		let bind: Vec<SocketAddr> = {
			let addr = if split[0].starts_with(':') {
				format!("0.0.0.0{}", split[0])
			} else {
				split[0].to_string()
			};
			addr.to_socket_addrs()
				.map_err(|_| AddressParse { input: addr })?
				.collect()
		};
		let target: Vec<SocketAddr> = {
			let addr = if split[1].starts_with(':') {
				format!("localhost{}", split[1])
			} else {
				split[1].to_string()
			};
			addr.to_socket_addrs()
				.map_err(|_| AddressParse { input: addr })?
				.collect()
		};

		// Try to find a common ground for net family
		let hosts = zip_filter(&bind, &target, |a| a.is_ipv6()).or_else(|| zip_filter(&bind, &target, |a| a.is_ipv4()));
		if let Some((source, target)) = hosts {
			return Ok(Thread {
				bind: *source,
				target: *target,
			});
		}

		Err(FamilyMix)
	}

	/// Consume the thread instance, moving it to its own dedicated os thread, only returning the handle which can
	/// later be used for shutting down the thread. (Using a signal handler)
	pub(super) fn spawn_into_handle(self) -> Result<ThreadHandle, Error> {
		// Bind the port so we can error early if need be
		let listener = StdTcpListener::bind(&self.bind).map_err(|_| Bind {
			addr: self.bind.to_string(),
		})?;

		// Then start a new thread
		let (sender, receiver) = oneshot::channel();
		let join_handle = thread::Builder::new()
			.name(format!("Listener-{}-to-{}", self.bind, self.target))
			.spawn(move || {
				// In which we set up the tokio core
				let core = Core::new().unwrap();
				let handle = core.handle();
				let listener = TcpListener::from_listener(listener, &self.bind, &handle).unwrap();
				self.listen(listener, core, handle, receiver);
			})
			.map_err(|_| ThreadStart)?;

		Ok(ThreadHandle::new(sender, join_handle))
	}

	fn listen(self, listener: TcpListener, core: Core, handle: Handle, shutdown: oneshot::Receiver<()>) {
		// Create a tokio task that accepts connections
		let task = listener
			.incoming()
			.for_each(|(client, client_addr)| {
				println!(
					"Setting up proxy from {} (bind: {}) to {}.",
					client_addr, self.bind, self.target
				);
				if let Err(e) = self.handle_client(client, &handle) {
					println!(
						"Could not set up proxy connection with from client {} (bind: {}) to target {}: {}",
						client_addr, self.bind, self.target, e
					);
				}
				Ok(())
			})
			.map_err(|e| Listen { cause: e.to_string() })
			.select(shutdown.map_err(|_| ChannelCancelled));

		let mut core = core;
		println!("Starting proxy from {} to {}.", self.bind, self.target);
		if let Err((e, _)) = core.run(task) {
			println!("Error while listening for proxy connections: {}", e);
		}
	}

	fn handle_client(&self, client: TcpStream, handle: &Handle) -> Result<(), Error> {
		let mut client = client;

		// Our target server will always be a server that accepts PROXY connections, so we need to figure out what to send.
		let proxy_header = {
			// Read first five bytes to see if the incoming connection has a PROXY header
			let mut header_buf = Vec::with_capacity(5);
			client
				.read_exact(header_buf.as_mut_slice())
				.map_err(|e| ProxyHeaderFailed { cause: e.to_string() })?;

			if header_buf != b"PROXY" {
				// No PROXY header, prepend a proxy header as if we're the first proxy

				// First, we check if we're on the correct family (PROXY does not allow family mixing)
				let local_addr = client.local_addr().unwrap();
				if local_addr.is_ipv4() != self.target.is_ipv4() {
					return Err(FamilyMix);
				}

				// Then prepare the header, and return it
				let mut header = format!(
					"PROXY TCP4 {} {} {} {}\r\n",
					local_addr.ip().to_string(),
					self.target.ip().to_string(),
					local_addr.port(),
					self.target.port(),
				)
				.into_bytes();
				header.extend_from_slice(&header_buf);
				header
			} else {
				// PROXY header detected, read the source IP and port
				let parts = {
					let mut header_buf = Vec::with_capacity(103);
					let mut single_buf = Vec::with_capacity(1);
					let single_buf = single_buf.as_mut_slice();

					// Keep reading PROXY header until we encounter a newline
					for _ in 0..header_buf.capacity() {
						client
							.read_exact(single_buf)
							.map_err(|e| ProxyHeaderFailed { cause: e.to_string() })?;
						if single_buf[0] == b'\n' {
							break;
						}
						header_buf.push(single_buf[0]);
					}

					// Then split the header in spaces
					String::from_utf8(header_buf)
						.map_err(|e| ProxyHeaderFailed { cause: e.to_string() })?
						.trim()
						.split(' ')
						.map(|s| s.to_string())
						.collect::<Vec<String>>()
				};

				// Check if we're not mixing different family types
				if (parts[0] == "TCP4" && !self.target.is_ipv4()) || (parts[0] == "TCP6" && !self.target.is_ipv6()) {
					return Err(FamilyMix);
				}

				format!(
					"PROXY TCP4 {} {} {} {}\r\n",
					parts[1],
					self.target.ip().to_string(),
					parts[3],
					self.target.port(),
				)
				.into_bytes()
			}
		};

		// Prepare the proxy task
		let proxy = TcpStream::connect(&self.target, handle)
			.and_then(move |target| {
				// Send PROXY header we just generated
				let mut target = target;
				target.write_all(&proxy_header).unwrap();

				// Set up actual proxy, for the traffic after the header
				let client_reader = AsyncTcpStream::from(client);
				let client_writer = client_reader.clone();
				let target_reader = AsyncTcpStream::from(target);
				let target_writer = target_reader.clone();

				// This task needs to shut down if either connection drops, one shutting down the other
				let client_to_server = copy(client_reader, target_writer)
					.and_then(|(n, _, mut server_writer)| server_writer.shutdown().map(move |_| n));
				let server_to_client = copy(target_reader, client_writer)
					.and_then(|(n, _, mut client_writer)| client_writer.shutdown().map(move |_| n));

				// And ensure this task waits for both connections
				client_to_server.join(server_to_client)
			})
			.map(|_| println!("Proxy connection closed"))
			.map_err(|e| println!("Forwarder connection errored: {}", e));
		handle.spawn(proxy);
		Ok(())
	}
}

/// Thread handle, when this goes out of scope the thread it represents is also shut down.
/// By the time Drop returns, the thread will have stopped.
pub(super) struct ThreadHandle {
	closer: Option<(oneshot::Sender<()>, JoinHandle<()>)>,
}

impl ThreadHandle {
	fn new(stop_sender: oneshot::Sender<()>, join_handle: JoinHandle<()>) -> Self {
		ThreadHandle {
			closer: Some((stop_sender, join_handle)),
		}
	}
}

impl Drop for ThreadHandle {
	fn drop(&mut self) {
		if let Some(closer) = self.closer.take() {
			match closer.0.send(()) {
				Err(_) => println!("Could send shut down signal to thread"),
				Ok(_) => closer.1.join().unwrap(),
			};
		}
	}
}

/// This function takes two slices, finding the first match for the filter from both, but only returning them if both
/// return at least one value.
fn zip_filter<T, F>(left: impl IntoIterator<Item = T>, right: impl IntoIterator<Item = T>, filter: F) -> Option<(T, T)>
where
	F: Fn(&T) -> bool,
{
	let left = left.into_iter().find(&filter);
	let right = right.into_iter().find(&filter);
	left.and_then(|s| right.map(|t| (s, t)))
}
