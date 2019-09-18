use std::fmt::{self, Debug, Display, Formatter};

#[derive(Fail)]
pub(super) enum Error {
	#[fail(display = "{}", usage)]
	Arguments { usage: String },
	#[fail(display = "Could not parse forwarder input: {}", input)]
	Argument { input: String },
	#[fail(display = "Could not parse bind address: {}", input)]
	AddressParse { input: String },
	#[fail(display = "Could not bind to address: {}", addr)]
	Bind { addr: String },
	#[fail(display = "Could not start thread (OS problem?)")]
	ThreadStart,
	#[fail(display = "Could not listen to connections: {}", cause)]
	Listen { cause: String },
	#[fail(display = "Shutdown channel was cancelled")]
	ChannelCancelled,
	#[fail(display = "Could not read PROXY header: {}", cause)]
	ProxyHeaderFailed { cause: String },
	#[fail(display = "Attempted to mix IPv4 and IPv6")]
	FamilyMix,
}

impl Debug for Error {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
		(self as &dyn Display).fmt(f)
	}
}
