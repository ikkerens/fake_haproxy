#[macro_use]
extern crate failure;

use {
	crate::{
		error::{Error, Error::Arguments},
		forwarder::Thread,
	},
	getopts::Options,
	std::{env, sync::mpsc},
};

mod async_stream;
mod error;
mod forwarder;

fn main() -> Result<(), Error> {
	// Configure the incoming arguments
	let mut options = Options::new();
	options.optmulti(
		"f",
		"forward",
		"Creates a forwarding tunnel",
		":80@:8080 or 127.0.0.1:80@192.168.1.2:8080",
	);

	// Gather passed arguments and parse them
	let matches = match options.parse(env::args()) {
		Err(_) => {
			return Err(Arguments {
				usage: options.usage("Could not parse arguments."),
			});
		}
		Ok(matches) => matches,
	};

	// Do we have any arguments passed?
	if !matches.opt_present("f") {
		return Err(Arguments {
			usage: options.usage("No forwarders provided."),
		});
	}

	// Set up forwarder instances
	let forwarders = {
		let mut result = Vec::new();
		for m in matches.opt_strs("f") {
			result.push(Thread::new(m)?);
		}
		result
	};

	// Spawn threads
	let mut cleanup = Vec::new();
	for f in forwarders {
		cleanup.push(f.spawn_into_handle()?);
	}

	// And wait for a signal before shutting down
	let (shutdown, shutdown_wait) = mpsc::channel();
	ctrlc::set_handler(move || shutdown.send(()).unwrap()).unwrap();
	shutdown_wait.recv().unwrap();

	Ok(())
}
